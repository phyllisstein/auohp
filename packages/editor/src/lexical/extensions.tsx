import { useEffect, useState, type JSX } from "react";
import { createPortal } from "react-dom";
import {
    $createTextNode,
    $getNodeByKey,
    $getRoot,
    $getSelection,
    $isRangeSelection,
    CLICK_COMMAND,
    COMMAND_PRIORITY_LOW,
    KEY_ENTER_COMMAND,
    configExtension,
    defineExtension,
    safeCast,
    type LexicalNode,
    type NodeKey,
} from "lexical";
import { namedSignals, type Signal } from "@lexical/extension";
import { HistoryExtension } from "@lexical/history";
import { RichTextExtension } from "@lexical/rich-text";
import { ReactExtension, type EditorChildrenComponentProps } from "@lexical/react/ReactExtension";
import { useExtensionComponent } from "@lexical/react/useExtensionComponent";
import { useExtensionSignalValue } from "@lexical/react/useExtensionSignalValue";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { $findMatchingParent } from "@lexical/utils";
import { debounce } from "es-toolkit/function";
import { playhead } from "@/playhead";
import {
    $createStatementNode,
    $createTagChipNode,
    $isStatementNode,
    BADGE_CLASS,
    StatementNode,
    TagChip,
    TagChipNode,
    TagMarkStyles,
} from "@/lexical/nodes";
import { INSERT_TAG_CHIP_COMMAND } from "@/lexical/commands";
import {
    SYNTHETIC_UID_MARKER,
    type EditStatementFn,
    type TranscriptStatements,
} from "@/lexical/shared";
import { $wrapSelectionInMarkNode, MarkExtension } from "@lexical/mark";

// -----------------------------------------------------------------------------
// Extensions --- Lexical's composition model as of 0.48.
//
// The OLD model (now deprecated) was: mount <LexicalComposer initialConfig={...}>
// and hang null-returning React components off it, each calling
// useLexicalComposerContext() to fish the editor back out of context and
// registering listeners in a useEffect. Behaviour was smuggled through the React
// tree, so the editor's capabilities were only knowable by reading JSX.
//
// The NEW model inverts that: an extension is a plain VALUE built by
// `defineExtension`. It declares what it contributes (`nodes`), what it needs
// (`dependencies`), what it can be tuned with (`config`), what it hands back
// (`build`), and what it does (`register`). The editor is a PARAMETER of
// `register`, not something retrieved from ambient context --- so the whole
// `useLexicalComposerContext` + `useEffect` + `return null` dance disappears.
// React re-enters only where there is genuinely something to paint.
// -----------------------------------------------------------------------------

// -----------------------------------------------------------------------------
// StatementExtension --- pure schema.
//
// It contributes StatementNode to the editor and nothing else. Under the old
// model this lived in a distant `initialConfig.nodes` array, structurally
// divorced from the plugins that used it; here the behavioural extensions below
// simply `dependencies: [StatementExtension]`, which both registers the node and
// documents the coupling. Listing it repeatedly is harmless --- the builder
// merges the dependency graph, so a node is registered once no matter how many
// extensions ask for it.
// -----------------------------------------------------------------------------
export const StatementExtension = /* @__PURE__ */ defineExtension({
    name: "@auohp/statement",
    nodes: () => [StatementNode],
});

// -----------------------------------------------------------------------------
// Litmus test 1 --- click-to-seek.
//
// Compare with the old StatementSeekPlugin: identical body, but the surrounding
// ceremony is gone. `register` receives the editor directly and returns the
// disposer that `editor.registerCommand` already hands back --- the same shape
// useEffect wanted, minus the component. Returning `false` from the handler
// leaves normal caret placement untouched; we are only observing.
// -----------------------------------------------------------------------------
export const StatementSeekExtension = /* @__PURE__ */ defineExtension({
    dependencies: [StatementExtension],
    name: "@auohp/statement-seek",
    register: editor =>
        editor.registerCommand(
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
                    console.debug(
                        `Clicked on statement: ${ statement.getUid() } (${ statement.getStartTime() })`,
                    );
                }

                return false;
            },
            COMMAND_PRIORITY_LOW,
        ),
});

// -----------------------------------------------------------------------------
// Litmus test 2 --- split-on-Enter. The feature Slate blocked on.
//
// COMMAND_PRIORITY_LOW (1) outranks RichTextExtension's default handler at
// COMMAND_PRIORITY_EDITOR (0), so returning `true` cleanly PRE-EMPTS the built-in
// paragraph insert. Note the dependency is on StatementExtension, not on
// RichTextExtension: priority ordering is a property of the command bus, not of
// registration order, so we do not have to sequence ourselves after rich text.
// -----------------------------------------------------------------------------
export const SplitStatementExtension = /* @__PURE__ */ defineExtension({
    dependencies: [StatementExtension],
    name: "@auohp/split-statement",
    register: editor =>
        editor.registerCommand(
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
        ),
});

// -----------------------------------------------------------------------------
// TagChipPortals --- the ElementNode -> React bridge.
//
// A DecoratorNode gets React for free: the reconciler PULLS JSX out of
// `decorate()` at the node's own position. TagChipNode extends MarkNode, hence
// ElementNode, so no such hook exists --- and it must stay an ElementNode,
// because its children ARE the tagged text (a DecoratorNode's getTextContent()
// returns "", which would silently erase those words from the statement text
// PersistenceExtension ships to the server).
//
// So we invert the flow and PUSH instead: a mutation listener reports every
// chip's lifecycle, we resolve each one's unmanaged badge span, and React
// portals into it. The badge being `setDOMUnmanaged` is what makes this legal
// --- Lexical's mutation-attribution up-walk terminates there, so nothing React
// renders inside gets evicted as foreign DOM.
//
// Rendered via ReactExtension's `decorators` channel, which exists precisely for
// "JSX inside the editor context that is not location-dependent".
// -----------------------------------------------------------------------------
function TagChipPortals(): JSX.Element {
    const [editor] = useLexicalComposerContext();

    // NodeKey -> the badge span to portal into. Held in React state (not a ref)
    // because adding or dropping an entry must trigger a re-render.
    const [hosts, setHosts] = useState<ReadonlyMap<NodeKey, HTMLElement>>(new Map());

    useEffect(
        () =>
            editor.registerMutationListener(TagChipNode, mutations => {
                // Resolve a chip's portal target from its NodeKey. Returns null
                // when the node has no DOM yet (or no badge, e.g. a node
                // replacement swapped createDOM out from under us).
                const resolveHost = (key: NodeKey): HTMLElement | null =>
                    editor.getElementByKey(key)?.querySelector<HTMLElement>(
                        `:scope > .${ BADGE_CLASS }`,
                    ) ?? null;

                setHosts(prev => {
                    let updates = 0;
                    const mutablePrev = new Map(prev);

                    for (const mutation of mutations) {
                        const [key, kind] = mutation;

                        if (kind === "updated") {
                            continue;
                        }

                        if (kind === "destroyed" && mutablePrev.has(key)) {
                            mutablePrev.delete(key);
                            updates++;
                            continue;
                        }

                        const host = resolveHost(key);

                        if (kind === "created" && !!host) {
                            mutablePrev.set(key, host);
                            updates++;
                        }
                    }

                    if (updates === 0) {
                        return prev;
                    }

                    return mutablePrev;
                });
            }),
        [editor],
    );

    return (
        <>
            { Array.from(hosts, ([key, host]) => createPortal(<TagChip nodeKey={ key } />, host, key)) }
        </>
    );
}

// -----------------------------------------------------------------------------
// TagChipExtension --- the React-in-editor seam.
//
// This is where the extension model pays off most visibly: the node and the
// command that inserts it are declared TOGETHER. Under the old model the node
// went in the composer's `initialConfig.nodes` and the command handler went in a
// <TagChipPlugin/> elsewhere in the JSX; nothing tied them together but
// convention and hope.
// -----------------------------------------------------------------------------
export const TagChipExtension = /* @__PURE__ */ defineExtension({
    name: "@auohp/tag-chip",
    nodes: () => [TagChipNode],
    dependencies: [
        MarkExtension,
        configExtension(ReactExtension, { decorators: [TagChipPortals] }),
    ],
    register: editor =>
        editor.registerCommand(
            INSERT_TAG_CHIP_COMMAND,
            id => {
                const selection = $getSelection();
                if (!$isRangeSelection(selection)) {
                    return false;
                }

                // `$wrapSelectionInMarkNode` does the whole selection -> element
                // wrap, including splitting boundary TextNodes. The 4th argument
                // is the factory hook that lets us substitute our subclass for a
                // plain MarkNode --- it receives the accumulated ids, so
                // overlapping tags merge rather than nest.
                $wrapSelectionInMarkNode(selection, false, id, ids => $createTagChipNode(ids));
                return true;
            },
            COMMAND_PRIORITY_LOW,
        ),
});

// -----------------------------------------------------------------------------
// PersistenceExtension --- the write path.
//
// Two things changed in the port beyond the mechanical de-componentisation:
//
// 1. It registers in `afterRegistration`, not `register`. Beware the tempting
//    inference here --- it is wrong, and it cost us a bug. The lifecycle IS
//    ordered `init -> build -> register -> InitialStateExtension.afterRegistration
//    -> ... -> afterRegistration`, but that ordering governs INITIATION, not
//    COMPLETION. InitialStateExtension seeds via `editor.update()`, and
//    `$beginUpdate` defers its commit to a MICROTASK
//    (`scheduleMicroTask(() => $commitPendingUpdates(editor))`), while
//    `LexicalBuilder.registerEditor` runs both of its loops SYNCHRONOUSLY.
//    Update listeners fire from `$commitPendingUpdates`, so the seed's dirty-node
//    wave lands after every `afterRegistration` has already returned --- and an
//    unguarded listener sees all of it: one spurious mutation per statement.
//
//    Tags would work (the seed carries HISTORY_MERGE_TAG), but see `lastPersisted`
//    below for why we ask a question about STATE instead of one about provenance.
//
// 2. `config` replaces props, and `build` turns that config into SIGNALS
//    (`namedSignals` --- the same move RichTextExtension and HistoryExtension
//    make). Reading `.peek()` at fire time rather than closing over a value means
//    the debounce delay, or the mutation function itself, can be retuned at
//    runtime from React via `useExtensionDependency(PersistenceExtension)`
//    WITHOUT tearing down and rebuilding the editor.
//
// The dirty-set -> statement mapping is unchanged from the plugin: Lexical has no
// operation stream (Slate's model), it hands us dirty NODE SETS per update, so we
// walk each dirty node up to its StatementNode ancestor and dedupe by NodeKey.
// -----------------------------------------------------------------------------
export interface PersistenceConfig {
    /** Apollo's `editStatement` executor. `null` disables persistence entirely. */
    editStatement: EditStatementFn | null;
    /** Per-statement debounce window, in milliseconds. */
    delay: number;
    /**
     * Text the server is known to hold, keyed by statement uid --- the same
     * `TranscriptStatements` payload `$initialEditorState` seeds the editor from.
     * Anything equal to this is, by definition, already persisted.
     */
    knownText: ReadonlyMap<string, string>;
}

export const PersistenceExtension = /* @__PURE__ */ defineExtension({
    config: /* @__PURE__ */ safeCast<PersistenceConfig>({
        delay: 1_000,
        editStatement: null,
        knownText: new Map<string, string>(),
    }),
    dependencies: [StatementExtension],
    name: "@auohp/persistence",

    // `build` MUST be declared before `afterRegistration`. TypeScript infers this
    // object literal's members in source order, so the Output type (produced here)
    // is only visible to `state.getOutput()` in members that come after it ---
    // otherwise it resolves to `unknown`. Lexical's own InitialStateExtension
    // carries a comment conceding the same ordering constraint.
    build: (_editor, config) => namedSignals(config),

    afterRegistration(editor, _config, state) {
        const { delay, editStatement, knownText } = state.getOutput();

        const createDebouncer = (uid: string) =>
            debounce((text: string) => {
                // FIXME: Fires for every single Statement on first being hydrated
                //
                // `.peek()` reads the signal without subscribing --- we want the
                // value as of the moment the debounce fires, not as of registration.
                // editStatement.peek()?.({
                //     variables: { uid, text },
                //     onCompleted: data => {
                //         console.debug(`Edit completed for statement ${ data.editStatement.uid }:`, data.editStatement);
                //     },
                // });
            }, delay.peek());

        // One debouncer PER STATEMENT UID, created lazily on first edit. The Slate
        // port used a single shared debouncer, which meant fast edits across two
        // statements cancelled each other's save --- a latent data-loss bug this
        // shape simply cannot have.
        const debouncers = new Map<string, ReturnType<typeof createDebouncer>>();

        // What the server is believed to hold, seeded from the same payload the
        // editor was built from. Copied (not aliased) because this map mutates as
        // we write, while the config signal stays the load-time snapshot.
        //
        // Comparing against this converts "was this update a user edit?" --- a
        // question about PROVENANCE, which update tags answer only approximately
        // --- into "is this text different from what the server has?", a question
        // about STATE. The second is strictly stronger: it also suppresses a
        // remount re-seed, an undo back to the original wording, and (later) a
        // collaborative echo, none of which carry HISTORY_MERGE_TAG.
        const lastPersisted = new Map(knownText.peek());

        const persist = (uid: string, text: string) => {
            // The whole bootstrap fix. Every statement in the seed wave arrives
            // with text identical to its `knownText` entry, so the entire wave
            // exits here without scheduling anything.
            if (lastPersisted.get(uid) === text) {
                return;
            }

            let flush = debouncers.get(uid);
            if (!flush) {
                flush = createDebouncer(uid);
                debouncers.set(uid, flush);
            }
            flush(text);
        };

        const unregister = editor.registerUpdateListener(
            ({ dirtyLeaves, dirtyElements, editorState }) => {
                if (dirtyLeaves.size === 0 && dirtyElements.size === 0) {
                    return;
                }

                editorState.read(() => {
                    const seen = new Set<NodeKey>();

                    const collect = (node: LexicalNode | null) => {
                        if (!node) {
                            return;
                        }
                        const statement = $isStatementNode(node)
                            ? node
                            : $findMatchingParent(node, $isStatementNode);
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
                        const dirtyLeaf = $getNodeByKey(key);
                        // console.log({ dirtyLeaf });
                        collect(dirtyLeaf);
                    }
                    for (const [key] of dirtyElements) {
                        const dirtyNode = $getNodeByKey(key);
                        // console.log({ dirtyNode });
                        collect(dirtyNode);
                    }
                });
            },
        );

        // The old plugin leaked here: its useEffect cleanup dropped the Map
        // without cancelling in-flight timers. An extension's disposer is the
        // natural place to do that properly.
        return () => {
            unregister();
            for (const flush of debouncers.values()) {
                flush.cancel();
            }
            debouncers.clear();
        };
    },
});

// -----------------------------------------------------------------------------
// LatencyExtension --- litmus test 1 instrumentation, and the canonical shape for
// "an extension that also has a face".
//
// `build` is the hook for producing values OTHER code consumes: it returns an
// `output` object, here a `stats` signal plus a `Component`. An extension whose
// output carries a `Component` satisfies `OutputComponentExtension`, which is
// exactly what `useExtensionComponent` consumes --- so React reaches the meter by
// asking the extension for it, rather than the meter reaching into React context
// for the editor. The dependency arrow reverses.
//
// Note also that `lastKeystrokeAt` is now a closure local rather than a module
// global. Under the old plugin, two editors on one page shared that variable and
// silently corrupted each other's measurements; per-editor `register` scope fixes
// that for free.
// -----------------------------------------------------------------------------
export interface LatencyStats {
    count: number;
    lastMs: number;
    maxMs: number;
}

export interface LatencyOutput {
    stats: Signal<LatencyStats>;
    Component: () => JSX.Element;
}

export const LatencyExtension = /* @__PURE__ */ defineExtension({
    build: (): LatencyOutput => ({
        ...namedSignals({ stats: { count: 0, lastMs: 0, maxMs: 0 } as LatencyStats }),
        Component: LatencyMeter,
    }),
    dependencies: [ReactExtension],
    name: "@auohp/latency",
    register(editor, _config, state) {
        const { stats } = state.getOutput();

        // Lexical is UNCONTROLLED: the ContentEditable owns its DOM and React does
        // not re-render per keystroke (the exact opposite of Slate's controlled
        // <Editable/>). To make that observable we stamp `performance.now()` on
        // each `beforeinput` and measure the gap to the resulting EditorState
        // update.
        let lastKeystrokeAt = 0;
        const stamp = () => {
            lastKeystrokeAt = performance.now();
        };

        const unregisterRoot = editor.registerRootListener((rootEl, prevRootEl) => {
            prevRootEl?.removeEventListener("beforeinput", stamp);
            rootEl?.addEventListener("beforeinput", stamp);
        });

        const unregisterUpdate = editor.registerUpdateListener(() => {
            if (lastKeystrokeAt === 0) {
                return;
            }
            const delta = performance.now() - lastKeystrokeAt;
            lastKeystrokeAt = 0;
            // Writing `.value` notifies subscribers; the React meter re-renders,
            // and NOTHING else does --- which is the property under test.
            const { count, maxMs } = stats.peek();
            stats.value = {
                count: count + 1,
                lastMs: delta,
                maxMs: Math.max(maxMs, delta),
            };
        });

        return () => {
            unregisterRoot();
            unregisterUpdate();
        };
    },
});

function LatencyMeter(): JSX.Element {
    // `useExtensionSignalValue` bridges the signal to React via
    // useSyncExternalStore --- no editor, no context, no useEffect.
    const stats = useExtensionSignalValue(LatencyExtension, "stats");

    return (
        <div style={{ fontFamily: "monospace", fontSize: "0.8rem", opacity: 0.8 }}>
            edits: { stats.count } | last: { stats.lastMs.toFixed(2) }ms | max: { stats.maxMs.toFixed(2) }ms
        </div>
    );
}

// A trivial toolbar affordance that dispatches the typed insert command ---
// demonstrating an out-of-editor React control mutating EditorState, which then
// renders back through a React DecoratorNode. `useLexicalComposerContext` is NOT
// deprecated: it remains the sanctioned way for a component that genuinely
// renders something to reach the editor (useExtensionComponent is built on it).
// What was deprecated is using it as a back door for behaviour-only components.
function TagButton(): JSX.Element {
    const [editor] = useLexicalComposerContext();

    // The payload is the MARK ID. Deferred decision: a throwaway uid for now, so
    // each chip is at least distinct. When entity resolution lands, this becomes
    // the graph uid of the Person/Organization being mentioned --- MarkNode's
    // __ids then genuinely means "this range MENTIONS these entities", and
    // $getMarkIDs answers that question directly. No signature change needed.
    return (
        <button
            type="button"
            onClick={ () => editor.dispatchCommand(INSERT_TAG_CHIP_COMMAND, crypto.randomUUID()) }>
            Insert #person chip
        </button>
    );
}

// ReactExtension renders `<>{contentEditable}{children}</>` by default, which
// would put our toolbar BELOW the transcript. Overriding EditorChildrenComponent
// through `configExtension` is how the new model does layout composition --- the
// editor's chrome is configuration of an extension rather than JSX the route
// happens to nest in the right order.
function EditorChrome({ contentEditable, children }: EditorChildrenComponentProps): JSX.Element {
    const Meter = useExtensionComponent(LatencyExtension);

    return (
        <>
            <TagMarkStyles />
            <div style={{ display: "flex", gap: "1rem", alignItems: "center", padding: "0.5rem 0" }}>
                <TagButton />
                <Meter />
            </div>
            { contentEditable }
            { children }
        </>
    );
}

// -----------------------------------------------------------------------------
// The root extension.
//
// This single value replaces the entire old `initialConfig` object AND the pile
// of <Plugin/> children: `namespace`/`onError` were initialConfig fields,
// `dependencies` were JSX children, `nodes` are contributed by the dependencies
// themselves, and `$initialEditorState` replaces SeedPlugin outright.
//
// It is a FACTORY rather than a constant because the seed data is per-interview.
// Because `$initialEditorState` is declared here it closes directly over
// `statements` --- no config plumbing, no `$getExtensionDependency` lookup.
// -----------------------------------------------------------------------------
export interface AuohpEditorOptions {
    statements: TranscriptStatements;
    editStatement: EditStatementFn;
}

export function defineAuohpEditorExtension({ statements, editStatement }: AuohpEditorOptions) {
    return defineExtension({
        dependencies: [
            RichTextExtension,
            HistoryExtension,
            StatementSeekExtension,
            SplitStatementExtension,
            TagChipExtension,
            LatencyExtension,
            configExtension(PersistenceExtension, { editStatement }),
            configExtension(ReactExtension, { EditorChildrenComponent: EditorChrome }),
        ],
        name: "@auohp/editor",
        namespace: "auohp-lexical-spike",
        onError: (error: Error) => {
            throw error;
        },

        // Runs once, inside an `editor.update()` tagged for history-merge, AFTER
        // every extension's `register` and BEFORE any `afterRegistration`. That
        // ordering is what lets PersistenceExtension drop the old "seed" tag check.
        $initialEditorState() {
            const root = $getRoot();
            root.clear();

            // Extending SerializedElementNode ultimately makes it more
            // difficult than creating them one by one
            //
            // let sn = StatementNode.importJSON(statement);

            // This is also not a thing: as above, type requires more than a
            // big bag of node JSON.
            //
            // RootNode.importJSON(statements);

            for (const statement of statements) {
                const statementNode = $createStatementNode(
                    statement.uid,
                    statement.startTime,
                    statement.endTime,
                );
                const textNode = $createTextNode(statement.text);
                statementNode.append(textNode);
                root.append(statementNode);
            }

            return root;
        },
    });
}
