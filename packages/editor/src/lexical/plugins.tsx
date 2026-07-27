import { useEffect, useState, type JSX } from "react";
import {
    $createTextNode,
    $getNodeByKey,
    $getRoot,
    $getSelection,
    $isRangeSelection,
    CLICK_COMMAND,
    COMMAND_PRIORITY_LOW,
    KEY_ENTER_COMMAND,
    type LexicalNode,
    type NodeKey,
} from "lexical";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { $findMatchingParent } from "@lexical/utils";
import { debounce } from "es-toolkit/function";
import { playhead } from "@/playhead";
import { $createStatementNode, $createTagChipNode, $isStatementNode } from "@/lexical/nodes";
import { INSERT_TAG_CHIP_COMMAND } from "@/lexical/commands";
import { SYNTHETIC_UID_MARKER, type EditStatementFn, type TranscriptStatements } from "@/lexical/shared";


// -----------------------------------------------------------------------------
// Litmus test 1 instrumentation --- edit latency.
//
// Lexical is UNCONTROLLED: the ContentEditable owns its own DOM and React does
// not re-render on every keystroke (the exact opposite of Slate's controlled
// <Editable/>, which round-trips every operation through React). To make the
// difference observable we stamp `performance.now()` on each `beforeinput` and
// measure the gap to the resulting EditorState update. This module-scoped
// mutable is the cheapest possible channel between the two listeners in
// LatencyMeter --- no signal, no state.
// -----------------------------------------------------------------------------
let lastKeystrokeAt = 0;


// -----------------------------------------------------------------------------
// Plugins. Each is a headless component that grabs the editor from context and
// registers listeners/commands in an effect --- Lexical's composition model.
// -----------------------------------------------------------------------------

// Seed the initial EditorState from the loaded statements. Tagged "seed" so the
// persistence and latency plugins can distinguish this synthetic first update
// from a real human edit.
export function SeedPlugin({ statements }: { statements: TranscriptStatements }): null {
    const [editor] = useLexicalComposerContext();

    useEffect(() => {
        editor.update(() => {
            const root = $getRoot();
            root.clear();

            for (const statement of statements) {
                const node = $createStatementNode(statement.uid, statement.startTime, statement.endTime);
                node.append($createTextNode(statement.text ?? ""));
                root.append(node);
            }
        }, { tag: "seed" });
        // `statements` is stable per interview; the composer is keyed by
        // interview.uid, so a fresh editor + fresh seed accompany each interview.
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [editor]);

    return null;
}


// Litmus test 1 --- click-to-seek. Registering CLICK_COMMAND is Lexical's
// documented click hook. We read the post-click selection, walk up to the
// enclosing StatementNode, and drive the shared playhead. Returning `false`
// leaves normal caret placement untouched --- we are only observing.
export function StatementSeekPlugin(): null {
    const [editor] = useLexicalComposerContext();

    useEffect(() => {
        return editor.registerCommand(
            CLICK_COMMAND,
            () => {
                const selection = $getSelection();
                if (!$isRangeSelection(selection)) {
                    return false;
                }

                const anchor = selection.anchor.getNode();
                const statement = $isStatementNode(anchor)
                    ? anchor
                    : $findMatchingParent(anchor, $isStatementNode);

                if ($isStatementNode(statement) && statement.getStartTime() != null) {
                    playhead.seek.value = statement.getStartTime()!;
                    console.debug(`Clicked on statement: ${ statement.getUid() } (${ statement.getStartTime() })`);
                }

                return false;
            },
            COMMAND_PRIORITY_LOW,
        );
    }, [editor]);

    return null;
}


// Litmus test 2 --- split-on-Enter. This is the feature Slate blocked on. Here
// it is a single registerCommand call against KEY_ENTER_COMMAND at
// COMMAND_PRIORITY_LOW: LOW (1) outranks the RichTextPlugin default at EDITOR (0),
// so returning `true` cleanly PRE-EMPTS the built-in paragraph/linebreak insert.
// No monkey-patching an undocumented `insertBreak` --- the keybinding is a
// public, typed command.
export function SplitStatementPlugin(): null {
    const [editor] = useLexicalComposerContext();

    useEffect(() => {
        return editor.registerCommand(
            KEY_ENTER_COMMAND,
            (event: KeyboardEvent | null) => {
                const selection = $getSelection();
                if (!$isRangeSelection(selection)) {
                    return false;
                }

                const anchorNode = selection.anchor.getNode();
                const statement = $isStatementNode(anchorNode)
                    ? anchorNode
                    : $findMatchingParent(anchorNode, $isStatementNode);

                if (!$isStatementNode(statement)) {
                    return false;
                }

                event?.preventDefault();

                // Caret offset relative to the whole statement: sum the text sizes
                // of children before the anchor, then add the in-node offset. This
                // stays correct even if Lexical has split the text into several
                // TextNodes.
                let caretOffset = selection.anchor.offset;
                let accumulated = 0;
                for (const child of statement.getChildren()) {
                    if (child.getKey() === anchorNode.getKey()) {
                        caretOffset = accumulated + selection.anchor.offset;
                        break;
                    }
                    accumulated += child.getTextContentSize();
                }

                const fullText = statement.getTextContent();
                const head = fullText.slice(0, caretOffset);
                const tail = fullText.slice(caretOffset);

                // Rebuild the current statement as `head`...
                for (const child of statement.getChildren()) {
                    child.remove();
                }
                statement.append($createTextNode(head));

                // ...and spill `tail` into a NEW, visual-only statement. Its uid is
                // synthetic (see SYNTHETIC_UID_MARKER) because no backend mutation
                // can mint a real one yet; it inherits the source caption window.
                const newStatement = $createStatementNode(
                    `${ statement.getUid() }${ SYNTHETIC_UID_MARKER }${ Date.now() }`,
                    statement.getStartTime(),
                    statement.getEndTime(),
                );
                newStatement.append($createTextNode(tail));
                statement.insertAfter(newStatement);
                newStatement.selectStart();

                return true;
            },
            COMMAND_PRIORITY_LOW,
        );
    }, [editor]);

    return null;
}


// The persistence porting seam. Slate handed us an operation stream and we
// keyed the debounce off `operation.type.includes("text")`. Lexical has NO
// operation stream --- it hands us DIRTY NODE SETS per update. So we invert the
// approach: on each update, map dirty leaves/elements up to their StatementNode
// ancestors, dedupe by uid, and fire a PER-UID debounced editStatement. Per-uid
// debouncers (vs Slate's single shared one) are strictly more correct: fast
// edits across two statements no longer cancel each other's save.
export function PersistencePlugin({ editStatement }: { editStatement: EditStatementFn }): null {
    const [editor] = useLexicalComposerContext();

    useEffect(() => {
        const debouncers = new Map<string, (text: string) => void>();

        const persist = (uid: string, text: string) => {
            let flush = debouncers.get(uid);
            if (!flush) {
                flush = debounce((latest: string) => {
                    editStatement({
                        variables: { uid, text: latest },
                        onCompleted: data => {
                            console.debug(`Edit completed for statement ${ data.editStatement.uid }:`, data.editStatement);
                        },
                    });
                }, 1_000);
                debouncers.set(uid, flush);
            }
            flush(text);
        };

        return editor.registerUpdateListener(({ dirtyLeaves, dirtyElements, editorState, tags }) => {
            // Skip the synthetic seed update and pure-selection updates.
            if (tags.has("seed")) {
                return;
            }
            if (dirtyLeaves.size === 0 && dirtyElements.size === 0) {
                return;
            }

            editorState.read(() => {
                const seen = new Set<NodeKey>();

                const collect = (node: LexicalNode | null) => {
                    if (!node) {
                        return;
                    }
                    const statement = $isStatementNode(node) ? node : $findMatchingParent(node, $isStatementNode);
                    if (!$isStatementNode(statement) || seen.has(statement.getKey())) {
                        return;
                    }
                    seen.add(statement.getKey());

                    const uid = statement.getUid();
                    // Visual-only split products have no backend row --- skip them.
                    if (uid.includes(SYNTHETIC_UID_MARKER)) {
                        return;
                    }
                    persist(uid, statement.getTextContent());
                };

                for (const key of dirtyLeaves) {
                    collect($getNodeByKey(key));
                }
                for (const [key] of dirtyElements) {
                    collect($getNodeByKey(key));
                }
            });
        });
    }, [editor, editStatement]);

    return null;
}


// Wires the INSERT_TAG_CHIP_COMMAND token to the actual node mutation.
export function TagChipPlugin(): null {
    const [editor] = useLexicalComposerContext();

    useEffect(() => {
        return editor.registerCommand(
            INSERT_TAG_CHIP_COMMAND,
            label => {
                const selection = $getSelection();
                if ($isRangeSelection(selection)) {
                    selection.insertNodes([$createTagChipNode(label)]);
                }
                return true;
            },
            COMMAND_PRIORITY_LOW,
        );
    }, [editor]);

    return null;
}


// Litmus test 1 readout. Subscribes to updates and reports the beforeinput ->
// EditorState-update latency. Only THIS tiny component re-renders per keystroke
// --- the editor surface itself does not --- which is precisely the property we
// are testing against Slate.
export function LatencyMeter(): JSX.Element {
    const [editor] = useLexicalComposerContext();
    const [stats, setStats] = useState({ count: 0, lastMs: 0, maxMs: 0 });

    useEffect(() => {
        const stamp = () => {
            lastKeystrokeAt = performance.now();
        };

        // Attach to the live root element; re-attach if the editor swaps roots.
        const unregisterRoot = editor.registerRootListener((rootEl, prevRootEl) => {
            prevRootEl?.removeEventListener("beforeinput", stamp);
            rootEl?.addEventListener("beforeinput", stamp);
        });

        const unregisterUpdate = editor.registerUpdateListener(({ tags }) => {
            if (tags.has("seed") || lastKeystrokeAt === 0) {
                return;
            }
            const delta = performance.now() - lastKeystrokeAt;
            lastKeystrokeAt = 0;
            setStats(prev => ({
                count: prev.count + 1,
                lastMs: delta,
                maxMs: Math.max(prev.maxMs, delta),
            }));
        });

        return () => {
            unregisterRoot();
            unregisterUpdate();
        };
    }, [editor]);

    return (
        <div style={{ fontFamily: "monospace", fontSize: "0.8rem", opacity: 0.8 }}>
            edits: { stats.count } | last: { stats.lastMs.toFixed(2) }ms | max: { stats.maxMs.toFixed(2) }ms
        </div>
    );
}
