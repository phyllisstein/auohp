import { createFileRoute } from "@tanstack/react-router";
import { useEffect, useRef, useState, type JSX, type RefObject } from "react";
import {
    $applyNodeReplacement,
    $createTextNode,
    $getNodeByKey,
    $getRoot,
    $getSelection,
    $isRangeSelection,
    CLICK_COMMAND,
    COMMAND_PRIORITY_LOW,
    createCommand,
    DecoratorNode,
    ElementNode,
    KEY_ENTER_COMMAND,
    type EditorConfig,
    type LexicalCommand,
    type LexicalEditor,
    type LexicalNode,
    type NodeKey,
    type SerializedElementNode,
    type SerializedLexicalNode,
    type Spread,
} from "lexical";
import { LexicalComposer } from "@lexical/react/LexicalComposer";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { RichTextPlugin } from "@lexical/react/LexicalRichTextPlugin";
import { ContentEditable } from "@lexical/react/LexicalContentEditable";
import { HistoryPlugin } from "@lexical/react/LexicalHistoryPlugin";
import { LexicalErrorBoundary } from "@lexical/react/LexicalErrorBoundary";
import { $findMatchingParent } from "@lexical/utils";
import { useMutation, useReadQuery } from "@apollo/client/react";
import { debounce } from "es-toolkit/function";
import { useSignalEffect } from "@preact/signals-react";
import { createGlobalStyle } from "styled-components";
import { playhead } from "@/playhead";
import type { EditStatementMutation, EditStatementMutationVariables, TranscriptQuery } from "@/gql/graphql";
// Reuse (do NOT duplicate) the incumbent Slate route's GraphQL operations and
// loader shape so the two editors talk to the backend identically. The only
// honest way to compare Slate vs Lexical is to hold everything else constant.
import { EDIT_STATEMENT_MUTATION, TRANSCRIPT_QUERY } from "./interview.$interviewNumber";


// FIXME: Constructing URLs for the caption endpoint and the public video URI
// should be a server-side concern (return a Video node, return Caption metadata).
const {
    VITE_AUOHP_PUBLIC: AUOHP_PUBLIC,
    VITE_AUOHP_API_URI: AUOHP_API_URI,
} = import.meta.env;


// A split-on-Enter produces a SECOND statement the backend knows nothing about:
// there is no `splitStatement`/`createStatement` mutation, only `editStatement`
// keyed by an existing uid. We tag synthetic uids with this marker so the
// persistence seam can recognise --- and skip --- statements that would 404.
// BACKEND GAP: a `splitStatement(uid, atOffset)` mutation would let these persist.
const SYNTHETIC_UID_MARKER = "::split-";


// -----------------------------------------------------------------------------
// Litmus test 1 instrumentation --- edit latency.
//
// Lexical is UNCONTROLLED: the ContentEditable owns its own DOM and React does
// not re-render on every keystroke (the exact opposite of Slate's controlled
// <Editable/>, which round-trips every operation through React). To make the
// difference observable we stamp `performance.now()` on each `beforeinput` and
// measure the gap to the resulting EditorState update. This module-scoped
// mutable is the cheapest possible cross-plugin channel --- no signal, no state.
// -----------------------------------------------------------------------------
let lastKeystrokeAt = 0;


const formatTimestamp = (timestamp: number) =>
    Temporal.Duration.from({ seconds: Math.round(timestamp) })
        .round({
            largestUnit: "hours",
            smallestUnit: "seconds",
        })
        .toLocaleString("en-US", {
            style: "digital",
            hoursDisplay: "auto",
            hours: "numeric",
        });


// -----------------------------------------------------------------------------
// StatementNode --- the Lexical analogue of the Slate `statement` element.
//
// It extends ElementNode (a block that HOLDS the editable TextNode children) and
// carries the graph identity (uid) plus the caption window (startTime/endTime).
// The non-editable timestamp label is drawn with a CSS `::before` pseudo-element
// fed by a `data-label` attribute --- pseudo-elements are inherently
// unselectable/uneditable, so Lexical's child reconciler never sees an extra DOM
// node to trip over. (Injecting a real <div> child into createDOM would desync
// the reconciler's child-index bookkeeping; the pseudo-element sidesteps that.)
// -----------------------------------------------------------------------------
type SerializedStatementNode = Spread<
    {
        uid: string;
        startTime: number | null;
        endTime: number | null;
    },
    SerializedElementNode
>;


class StatementNode extends ElementNode {
    __uid: string;
    __startTime: number | null;
    __endTime: number | null;

    static getType(): string {
        return "statement";
    }

    // `clone` is how Lexical produces the next immutable version of a node during
    // an update --- it MUST copy every custom field AND the key, or edits to one
    // version silently drop the graph identity of the next.
    static clone(node: StatementNode): StatementNode {
        return new StatementNode(node.__uid, node.__startTime, node.__endTime, node.__key);
    }

    static importJSON(serialized: SerializedStatementNode): StatementNode {
        return $createStatementNode(serialized.uid, serialized.startTime, serialized.endTime);
    }

    constructor(uid: string, startTime: number | null, endTime: number | null, key?: NodeKey) {
        super(key);
        this.__uid = uid;
        this.__startTime = startTime;
        this.__endTime = endTime;
    }

    // Getters route through `getLatest()` so reads always see the current version
    // of the node within an update, never a stale snapshot captured by closure.
    getUid(): string {
        return this.getLatest().__uid;
    }

    getStartTime(): number | null {
        return this.getLatest().__startTime;
    }

    getEndTime(): number | null {
        return this.getLatest().__endTime;
    }

    createDOM(_config: EditorConfig): HTMLElement {
        const dom = document.createElement("div");
        dom.className = "auohp-statement";
        dom.setAttribute("data-uid", this.__uid);
        dom.setAttribute("data-label", this.#label());
        return dom;
    }

    // Return `false` --- Lexical keeps managing our text children --- but first
    // refresh the label if the caption window shifted (e.g. an inherited split).
    updateDOM(prevNode: StatementNode, dom: HTMLElement): boolean {
        if (prevNode.__startTime !== this.__startTime || prevNode.__endTime !== this.__endTime) {
            dom.setAttribute("data-label", this.#label());
        }
        return false;
    }

    exportJSON(): SerializedStatementNode {
        return {
            ...super.exportJSON(),
            type: "statement",
            version: 1,
            uid: this.__uid,
            startTime: this.__startTime,
            endTime: this.__endTime,
        };
    }

    #label(): string {
        const start = this.__startTime ?? 0;
        const end = this.__endTime ?? 0;
        return `${ formatTimestamp(start) } - ${ formatTimestamp(end) }`;
    }
}


function $createStatementNode(uid: string, startTime: number | null, endTime: number | null): StatementNode {
    // `$applyNodeReplacement` runs any registered node-replacement hooks and is
    // the idiomatic constructor wrapper --- cheap insurance even when we register
    // no replacements today.
    return $applyNodeReplacement(new StatementNode(uid, startTime, endTime));
}


function $isStatementNode(node: LexicalNode | null | undefined): node is StatementNode {
    return node instanceof StatementNode;
}


// -----------------------------------------------------------------------------
// Litmus test 3 --- React-in-editor.
//
// TagChipNode is a DecoratorNode: a leaf whose visual body is a REACT component
// rendered by @lexical/react's decorator machinery. This is the primitive future
// graph-tagging needs --- an inline node that lives in EditorState (serialises,
// undoes, participates in selection) yet paints itself with arbitrary React.
// `decorate()` is the state -> React bridge: whatever it returns is mounted into
// the editor's DOM at this node's position and re-rendered when the node changes.
// -----------------------------------------------------------------------------
type SerializedTagChipNode = Spread<{ label: string }, SerializedLexicalNode>;


function TagChip({ label }: { label: string }) {
    return (
        <span
            style={{
                display: "inline-block",
                padding: "0 0.4em",
                margin: "0 0.15em",
                borderRadius: "0.6em",
                fontSize: "0.85em",
                fontWeight: 600,
                color: "#0b0b0b",
                background: "#7dd3fc",
                userSelect: "none",
            }}>
            #{ label }
        </span>
    );
}


class TagChipNode extends DecoratorNode<JSX.Element> {
    __label: string;

    static getType(): string {
        return "tag-chip";
    }

    static clone(node: TagChipNode): TagChipNode {
        return new TagChipNode(node.__label, node.__key);
    }

    static importJSON(serialized: SerializedTagChipNode): TagChipNode {
        return $createTagChipNode(serialized.label);
    }

    constructor(label: string, key?: NodeKey) {
        super(key);
        this.__label = label;
    }

    // Inline so the chip flows within a statement's text rather than breaking it
    // onto its own line.
    isInline(): boolean {
        return true;
    }

    createDOM(): HTMLElement {
        const dom = document.createElement("span");
        dom.style.display = "inline-block";
        return dom;
    }

    // The DOM host never changes --- React owns everything inside it --- so the
    // reconciler can skip this node entirely.
    updateDOM(): boolean {
        return false;
    }

    exportJSON(): SerializedTagChipNode {
        return {
            ...super.exportJSON(),
            type: "tag-chip",
            version: 1,
            label: this.__label,
        };
    }

    decorate(): JSX.Element {
        return <TagChip label={ this.__label } />;
    }
}


function $createTagChipNode(label: string): TagChipNode {
    return $applyNodeReplacement(new TagChipNode(label));
}


// A typed command is the discoverable, first-class way to expose an editor
// action --- `createCommand<Payload>()` gives us a token any component can
// `dispatchCommand(TOKEN, payload)` against, decoupling the toolbar button from
// the node-mutation logic below.
const INSERT_TAG_CHIP_COMMAND: LexicalCommand<string> = createCommand("INSERT_TAG_CHIP_COMMAND");


// -----------------------------------------------------------------------------
// Plugins. Each is a headless component that grabs the editor from context and
// registers listeners/commands in an effect --- Lexical's composition model.
// -----------------------------------------------------------------------------

// Seed the initial EditorState from the loaded statements. Tagged "seed" so the
// persistence and latency plugins can distinguish this synthetic first update
// from a real human edit.
function SeedPlugin({ statements }: { statements: TranscriptStatements }): null {
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
function StatementSeekPlugin(): null {
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
function SplitStatementPlugin(): null {
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
function PersistencePlugin({ editStatement }: { editStatement: EditStatementFn }): null {
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
function TagChipPlugin(): null {
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
function LatencyMeter(): JSX.Element {
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


// Drives the <video> from the shared playhead. Identical machinery to the Slate
// route --- kept here rather than in a plugin because it needs the video ref.
function useVideoSync(player: RefObject<HTMLVideoElement | null>) {
    useSignalEffect(() => {
        !!player.current && (player.current.currentTime = playhead.seek.value);
    });
}


// Styles for the non-editable timestamp label (see StatementNode.createDOM).
const EditorStyle = createGlobalStyle`
    .auohp-statement {
        padding: 0.25rem 0;
    }

    .auohp-statement::before {
        display: block;
        font-family: monospace;
        font-size: 0.75rem;
        color: #888;
        content: attr(data-label);
        user-select: none;
    }
`;


// ---- GraphQL result-shape aliases (derived from the reused operations) --------
type TranscriptStatements = TranscriptQuery["interviewTranscript"]["statements"];
type EditStatementFn = ReturnType<typeof useMutation<EditStatementMutation, EditStatementMutationVariables>>[0];


// The trailing underscore on `$interviewNumber_` is TanStack's DE-NESTING
// convention: the URL stays `/interview/:n/lexical`, but the route opts OUT of
// rendering inside the Slate route (`interview.$interviewNumber.tsx`), which has
// no <Outlet/>. So this is a sibling under root, not a child --- the Slate route
// stays untouched. The generator maintains the `_` in this path string for us.
export const Route = createFileRoute("/interview/$interviewNumber_/lexical")({
    component: Page,
    // FIXME: Handle errors gracefully
    loader: ({ context: { preloadQuery }, params }) => {
        const interviewNumber = Number.parseInt(params.interviewNumber);
        if (Number.isNaN(interviewNumber)) {
            throw new Error("Invalid interview number");
        }

        const transcriptQueryRef = preloadQuery(TRANSCRIPT_QUERY, {
            variables: {
                interviewNumber,
            },
        });

        return { transcriptQueryRef };
    },
});


function Page() {
    const interviewNumber = Number.parseInt(Route.useParams().interviewNumber);
    const { transcriptQueryRef } = Route.useLoaderData();
    const { data: transcriptData } = useReadQuery(transcriptQueryRef);

    const [editStatement] = useMutation(EDIT_STATEMENT_MUTATION);

    const player = useRef<HTMLVideoElement>(null);
    useVideoSync(player);

    const { statements, interview } = transcriptData.interviewTranscript;

    const initialConfig = {
        namespace: "auohp-lexical-spike",
        // Registering our custom nodes is how Lexical learns to construct,
        // serialise, and reconcile them. Forget one and the editor throws on the
        // first attempt to create it --- a loud, discoverable failure mode.
        nodes: [StatementNode, TagChipNode],
        onError: (error: Error) => {
            throw error;
        },
        // We seed via SeedPlugin (tagged "seed") rather than initialConfig, so the
        // persistence/latency listeners can tell the seed apart from real edits.
        theme: {},
    };

    return (
        <div>
            <EditorStyle />
            <video
                ref={ player }
                controls
                crossOrigin="anonymous"
                onTimeUpdate={ ev => playhead.timestamp.value = (ev.target as HTMLVideoElement).currentTime }>
                <source src={ `${ AUOHP_PUBLIC }/videos/${ interviewNumber }.mp4` } type="video/mp4" />
                {
                    interview.uid && <track kind="captions" src={ `${ AUOHP_API_URI }/interview/${ interview.uid }/vtt` } srcLang="en" label="English" />
                }
            </video>

            {/* `key` remounts the whole composer (fresh editor + fresh seed) when
                the interview changes --- mirrors the Slate route's `key`. */}
            <LexicalComposer key={ interview.uid } initialConfig={ initialConfig }>
                <div style={{ display: "flex", gap: "1rem", alignItems: "center", padding: "0.5rem 0" }}>
                    <TagButton />
                    <LatencyMeter />
                </div>

                <RichTextPlugin
                    contentEditable={ <ContentEditable /> }
                    ErrorBoundary={ LexicalErrorBoundary } />
                <HistoryPlugin />
                <SeedPlugin statements={ statements } />
                <StatementSeekPlugin />
                <SplitStatementPlugin />
                <TagChipPlugin />
                <PersistencePlugin editStatement={ editStatement } />
            </LexicalComposer>
        </div>
    );
}


// A trivial toolbar affordance that dispatches the typed insert command ---
// demonstrating an out-of-editor React control mutating EditorState, which then
// renders back through a React DecoratorNode.
function TagButton(): JSX.Element {
    const [editor] = useLexicalComposerContext();

    return (
        <button
            type="button"
            onClick={ () => editor.dispatchCommand(INSERT_TAG_CHIP_COMMAND, "person") }>
            Insert #person chip
        </button>
    );
}
