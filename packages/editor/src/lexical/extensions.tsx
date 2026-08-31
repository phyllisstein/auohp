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

import { $findMatchingParent, mergeRegister } from "@lexical/utils";
import { debounce } from "es-toolkit/function";
import { playhead } from "@/playhead";
import {
    $createStatementNode,
    $createTagChipNode,
    $isStatementNode,
    $isTagChipNode,
    $isSearchResultNode,
    $createSearchResultNode,
    TAG_CHIP_BADGE_CLASS,
    StatementNode,
    TagChip,
    TagChipNode,
    TagMarkStyles,
    SearchResult,
    SearchResultNode,
    SEARCH_RESULT_BADGE_CLASS,
} from "@/lexical/nodes";
import { INSERT_TAG_CHIP_COMMAND, INSERT_SEARCH_RESULT_COMMAND, PERFORM_SEARCH_COMMAND } from "@/lexical/commands";
import {
    SYNTHETIC_UID_MARKER,
    type EditStatementFn,
    type TranscriptStatements,
    type SearchStatementsData,
} from "@/lexical/shared";
import { $isMarkNode, $unwrapMarkNode, $wrapSelectionInMarkNode, MarkExtension } from "@lexical/mark";
import { useExtensionSignalValue, useSignalValue } from "@lexical/react/useExtensionSignalValue";
import { SEARCH_STATEMENTS_QUERY } from "@/queries";
import { useLazyQuery } from "@apollo/client/react";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { useExtensionComponent, useExtensionDependency } from "@lexical/react/useExtensionComponent";
import { createPortal } from "react-dom";
import { useEffect, useState, type JSX, useEffectEvent } from "react";


// -----------------------------------------------------------------------------
// Extensions --- Lexical's composition model as of 0.48.
//
// The old model (now deprecated) was: mount <LexicalComposer initialConfig={...}>
// and hang null-returning React components off it, each calling
// useLexicalComposerContext() to fish the editor back out of context and
// registering listeners in a useEffect. Behaviour was smuggled through the React
// tree, so the editor's capabilities were only knowable by reading JSX.
//
// The new model inverts that: an extension is a plain value built by
// `defineExtension`. It declares what it contributes (`nodes`), what it needs
// (`dependencies`), what it can be tuned with (`config`), what it hands back
// (`build`), and what it does (`register`). The editor is a parameter of
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
// TagChipExtension --- the React-in-editor seam.
//
// This is where the extension model pays off most visibly: the node and the
// command that inserts it are declared together. Under the old model the node
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
// TagSplitBoundaryExtension --- keep proper nouns whole across caption breaks.
//
// StatementNode.insertNewAfter now handles the split itself, so the old
// SplitStatementExtension is gone. What remains is an editorial rule, not a
// structural one: a caption boundary should never fall inside a proper noun.
// Enter halfway through "Larry Kramer" should still split --- just not there.
//
// This has to run before anything mutates. `$removeTextAndSplitBlock` walks up
// splitting nodes until it reaches a block, and a chip is inline
// (INTERNAL_$isBlock requires !isInline()), so it cleaves the chip in two on its
// way to the statement. By the time insertNewAfter is called it is already too
// late to object.
//
// COMMAND_PRIORITY_LOW (1) runs before RichTextExtension's default handler at
// COMMAND_PRIORITY_EDITOR (0) --- the bus dispatches high-to-low. We relocate
// the caret and return `false`, so the default handler proceeds normally; it
// re-reads `$getSelection()` rather than closing over one, so it sees our edit.
// -----------------------------------------------------------------------------
export const TagSplitBoundaryExtension = /* @__PURE__ */ defineExtension({
    dependencies: [StatementExtension, TagChipExtension],
    name: "@auohp/tag-split-boundary",
    register: editor =>
        editor.registerCommand(
            KEY_ENTER_COMMAND,
            event => {
                const selection = $getSelection();

                // Non-collapsed selections replace their content before
                // splitting --- a different problem, left alone for now.
                if (!$isRangeSelection(selection) || !selection.isCollapsed()) {
                    return false;
                }

                const anchor = selection.anchor.getNode();
                const chip = $isTagChipNode(anchor)
                    ? anchor
                    : $findMatchingParent(anchor, $isTagChipNode);

                // Caret is not inside a proper noun --- nothing to consolidate.
                if (!$isTagChipNode(chip)) {
                    return false;
                }

                // FIXME: Show UX feedback or make a call on splitting the text
                // before/after the chip. For now, silently bail in all cases.
                event?.preventDefault();
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
//    inference here --- it is wrong, and it cost us a bug. The lifecycle is
//    ordered `init -> build -> register -> InitialStateExtension.afterRegistration
//    -> ... -> afterRegistration`, but that ordering governs initiation, not
//    completion. InitialStateExtension seeds via `editor.update()`, and
//    `$beginUpdate` defers its commit to a microtask
//    (`scheduleMicroTask(() => $commitPendingUpdates(editor))`), while
//    `LexicalBuilder.registerEditor` runs both of its loops synchronously.
//    Update listeners fire from `$commitPendingUpdates`, so the seed's dirty-node
//    wave lands after every `afterRegistration` has already returned --- and an
//    unguarded listener sees all of it: one spurious mutation per statement.
//
//    Tags would work (the seed carries HISTORY_MERGE_TAG), but see `lastPersisted`
//    below for why we ask a question about state instead of one about provenance.
//
// 2. `config` replaces props, and `build` turns that config into signals
//    (`namedSignals` --- the same move RichTextExtension and HistoryExtension
//    make). Reading `.peek()` at fire time rather than closing over a value means
//    the debounce delay, or the mutation function itself, can be retuned at
//    runtime from React via `useExtensionDependency(PersistenceExtension)`
//    without tearing down and rebuilding the editor.
//
// The dirty-set -> statement mapping is unchanged from the plugin: Lexical has no
// operation stream (Slate's model), it hands us dirty node sets per update, so we
// walk each dirty node up to its StatementNode ancestor and dedupe by NodeKey.
// -----------------------------------------------------------------------------
export interface PersistenceConfig {
    /** Apollo's `editStatement` executor. `null` disables persistence entirely. */
    editStatement: EditStatementFn | null;
    /** Per-statement debounce window, in milliseconds. */
    delay: number;
}

export const PersistenceExtension = /* @__PURE__ */ defineExtension({
    config: /* @__PURE__ */ safeCast<PersistenceConfig>({
        delay: 1_000,
        editStatement: null,
    }),
    dependencies: [StatementExtension],
    name: "@auohp/persistence",

    // `build` must be declared before `afterRegistration`. TypeScript infers this
    // object literal's members in source order, so the Output type (produced here)
    // is only visible to `state.getOutput()` in members that come after it ---
    // otherwise it resolves to `unknown`. Lexical's own InitialStateExtension
    // carries a comment conceding the same ordering constraint.
    build: (_editor, config) => namedSignals(config),

    afterRegistration (editor, _config, state) {
        const { delay, editStatement } = state.getOutput();

        const createDebouncer = (uid: string) =>
            debounce((text: string) => {
                // `.peek()` reads the signal without subscribing --- we want the
                // value as of the moment the debounce fires, not as of registration.
                editStatement.peek()?.({
                    variables: { uid, text },
                    onCompleted: data => {
                        console.debug(`Edit completed for statement ${ data.editStatement.uid }:`, data.editStatement);
                    },
                });
            }, delay.peek());

        // One debouncer PER STATEMENT UID, created lazily on first edit. The Slate
        // port used a single shared debouncer, which meant fast edits across two
        // statements cancelled each other's save --- a latent data-loss bug this
        // shape simply cannot have.
        const debouncers = new Map<string, ReturnType<typeof createDebouncer>>();

        const persist = (uid: string, text: string) => {
            let flush = debouncers.get(uid);
            if (!flush) {
                flush = createDebouncer(uid);
                debouncers.set(uid, flush);
            }
            flush(text);
        };

        const unregister = editor.registerUpdateListener(
            ({ dirtyLeaves, dirtyElements, editorState, tags }) => {
                if (tags.has("history-merge")) {
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
// SearchInterviewExtension --- the read path, and a worked example of the one
// mechanism that carries live data into an extension.
//
// The tempting-but-broken shape was to hand Apollo's `useLazyQuery` tuple to
// this extension as config. It cannot work, and the reason is worth internalising
// because it generalises to every "how do I update my extension" question:
//
//   `LexicalBuilder` calls `build(editor, config)` exactly once, at editor
//   construction. `namedSignals(config)` then copies each config value into a
//   fresh signal at that instant. There is no later re-apply pass, because there
//   is no re-render for an extension --- an extension is a value, not a
//   component. So config is the signal's seed; the signal is the channel.
//   `namedSignals`' own docstring concedes this: it exists "so it can be
//   reconfigured at runtime".
//
// Worse, trying to force it through config poisons the editor's lifetime. The
// route must `useMemo` the extension, `LexicalExtensionComposer` memoises the
// editor on that extension's identity and disposes the old one --- so putting a
// per-render Apollo result in the dep array rebuilds the editor mid-search and
// throws away the user's unsaved edits and caret.
//
// Hence: this extension takes no config and owns its query outright. Data flows
// one way, and every hop is a signal write:
//
//   PERFORM_SEARCH_COMMAND -> `query` signal
//                          -> SearchDriver (React) runs Apollo
//                          -> `data` / `loading` signals
//                          -> consumers (React, or register-time subscribers)
//
// The command handler stays a pure state write --- it never touches Apollo. That
// is what keeps it synchronous (Lexical command handlers must return a boolean
// immediately) and what makes "what does this command do" answerable without
// knowing anything about the network.
// -----------------------------------------------------------------------------
export interface SearchOutput {
    /** The pending search string. `null` means idle --- no search requested yet. */
    query: Signal<string | null>;
    /** Latest results, or `undefined` before the first response. */
    data: Signal<SearchStatementsData>;
    /** Whether a search is currently in flight. */
    loading: Signal<boolean>;
}

// The `$` prefix is Lexical's convention for "only callable inside an
// `editor.update()` or `editorState.read()` context" --- it is not a sigil the
// runtime understands, just a naming discipline that makes the requirement
// visible at the call site. Calling this outside an update throws.
//
// Contract: given a result set (or `undefined`, meaning "no search"), leave the
// document holding exactly the marks that set implies --- nothing stale from the
// previous search, nothing missing from this one.
function $applySearchResults (results: SearchStatementsData): void {
    const uids = new Set(results?.searchStatements.map(({ statement }) => statement.uid) ?? []);

    const root = $getRoot();
    const children = root.getChildren();
    console.log(`$applySearchResults: ${ children.length } root children, ${ uids.size } results`);

    // FIXME: This unwraps and rewraps unconditionally, so a statement that matched
    // the previous search and matches this one too still has its SearchResultNode
    // destroyed and recreated with an identical uid. That fires `destroyed` then
    // `created` on the mutation listener, so SearchResultPortals runs two setHosts
    // passes and React unmounts and remounts the badge portal. Invisible today
    // because SearchResult is stateless; it stops being invisible as soon as a
    // badge holds state (hover popover, animation, jump-to-result toggle). The fix
    // is to diff against the marks already present and leave unchanged ones alone.
    for (const child of children) {
        if ($isStatementNode(child)) {
            const statement = child;
            const grandchildren = statement.getChildren();
            for (const grandchild of grandchildren) {
                if ($isSearchResultNode(grandchild)) {
                    $unwrapMarkNode(grandchild);
                }
            }

            const uid = statement.getUid();
            if (uids.has(uid)) {
                console.log(`$applySearchResults: statement ${ uid } ${ uids.has(uid) ? "matches" : "does not match" }`);
                const mark = $createSearchResultNode([uid]);

                statement.getChildren().forEach(statementChild => {
                    mark.append(statementChild);
                });

                statement.append(mark);
            }
        }
    }
}

export const SearchInterviewExtension = /* @__PURE__ */ defineExtension({
    nodes: () => [SearchResultNode],
    dependencies: [
        StatementExtension,
        configExtension(ReactExtension, { decorators: [TagChipPortals, SearchResultPortals, SearchDriver] }),
    ],
    name: "@auohp/search-interview",

    // The return type is annotated rather than inferred, and that is load-bearing
    // for a reason that has nothing to do with documentation: `dependencies` above
    // names `SearchDriver`, and `SearchDriver`'s body asks for this extension's
    // output. Left to inference that is a cycle TypeScript refuses to resolve.
    // Annotating here (and annotating SearchDriver's return type) cuts it in both
    // directions --- neither side needs the other's body to compute its type.
    build: (): SearchOutput => namedSignals({
        query: null as string | null,
        data: undefined as SearchStatementsData,
        loading: false,
    }),

    register (editor, _config, state) {
        const { query, data } = state.getOutput();

        // Preact's `subscribe` invokes its callback immediately with the current
        // value. At registration that value is `undefined` and the document is not
        // even seeded yet ($initialEditorState runs after every `register`), so the
        // first call is noise. Swallowing it explicitly beats guarding on
        // `results === undefined` inside the subscriber, because `data` legitimately
        // returns to `undefined` later and that case must still clear the marks.
        let primed = false;

        // `mergeRegister` folds several disposers into one. The previous version
        // returned nothing from `register`, so both command handlers outlived the
        // editor --- revisiting the route stacked a second handler on the same
        // command, and one click fired two searches.
        return mergeRegister(
            // The read path's terminus, and note that no React is involved: signals
            // are subscribable anywhere, and `register` already holds the editor.
            // React appears in this extension only where something is painted
            // (SearchResultPortals) or where a hook is unavoidable (SearchDriver).
            data.subscribe(results => {
                if (!primed) {
                    primed = true;
                    return;
                }

                // `history-merge` is load-bearing, not decoration. Wrapping text in
                // a MarkNode leaves `getTextContent()` byte-identical, so
                // PersistenceExtension would happily fire an `editStatement` per
                // highlighted statement, saving text that never changed. It skips
                // this tag --- and search highlighting is genuinely not a user edit,
                // so it should not enter the undo stack as one either.
                editor.update(() => $applySearchResults(results), { tag: "history-merge" });
            }),

            editor.registerCommand(
                INSERT_SEARCH_RESULT_COMMAND,
                id => {
                    const selection = $getSelection();
                    if (!$isRangeSelection(selection)) {
                        return false;
                    }

                    // `$wrapSelectionInMarkNode` does the whole selection -> element
                    // wrap, including splitting boundary TextNodes. The 4th argument
                    // is the factory hook that lets us substitute our subclass for a
                    // plain MarkNode --- it receives the accumulated ids, so
                    // overlapping marks merge rather than nest.
                    $wrapSelectionInMarkNode(selection, false, id, ids => $createSearchResultNode(ids));
                    return true;
                },
                COMMAND_PRIORITY_LOW,
            ),

            editor.registerCommand(
                PERFORM_SEARCH_COMMAND,
                () => {
                    const selection = $getSelection();
                    if (!$isRangeSelection(selection)) {
                        return false;
                    }

                    const text = selection.getTextContent();
                    if (text.length === 0) {
                        return false;
                    }

                    // The entire effect of this command. Writing the signal is the
                    // request; SearchDriver is what makes it a network call.
                    query.value = text;
                    return true;
                },
                COMMAND_PRIORITY_LOW,
            ),
        );
    },
});


// -----------------------------------------------------------------------------
// LatencyExtension --- litmus test 1 instrumentation, and the canonical shape for
// "an extension that also has a face".
//
// `build` is the hook for producing values other code consumes: it returns an
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
    register (editor, _config, state) {
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
            // and nothing else does --- which is the property under test.
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


// -----------------------------------------------------------------------------
// The root extension.
//
// This single value replaces the entire old `initialConfig` object and the pile
// of <Plugin/> children: `namespace`/`onError` were initialConfig fields,
// `dependencies` were JSX children, `nodes` are contributed by the dependencies
// themselves, and `$initialEditorState` replaces SeedPlugin outright.
//
// It is a factory rather than a constant because the seed data is per-interview.
// Because `$initialEditorState` is declared here it closes directly over
// `statements` --- no config plumbing, no `$getExtensionDependency` lookup.
// -----------------------------------------------------------------------------
// Note what is not here any more: nothing search-related. Every option in this
// interface must be stable for the editor's entire lifetime, because the route
// has to memoise the returned extension and any change to it destroys the
// document. Live data belongs in signals, not here.
export interface AuohpEditorOptions {
    statements: TranscriptStatements;
    editStatement: EditStatementFn;
}

export function defineAuohpEditorExtension ({ statements, editStatement }: AuohpEditorOptions) {
    return defineExtension({
        dependencies: [
            configExtension(PersistenceExtension, { editStatement }),
            SearchInterviewExtension,
            configExtension(ReactExtension, { EditorChildrenComponent: EditorChrome }),
            HistoryExtension,
            LatencyExtension,
            RichTextExtension,
            StatementExtension,
            StatementSeekExtension,
            TagChipExtension,
            TagSplitBoundaryExtension,
        ],
        name: "@auohp/editor",
        namespace: "auohp-lexical-spike",
        onError: (error: Error) => {
            throw error;
        },

        // Runs once, inside an `editor.update()` tagged for history-merge, after
        // every extension's `register` and before any `afterRegistration`. That
        // ordering is what lets PersistenceExtension drop the old "seed" tag check.
        $initialEditorState () {
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

            const appendables: StatementNode[] = [];
            for (const statement of statements) {
                const statementNode = $createStatementNode(
                    statement.uid,
                    statement.startTime,
                    statement.endTime,
                );
                const textNode = $createTextNode(statement.text);
                statementNode.append(textNode);
                appendables.push(statementNode);
            }
            root.append(...appendables);

            return root;
        },
    });
}

function LatencyMeter (): JSX.Element {
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
// renders back through a React DecoratorNode. `useLexicalComposerContext` is not
// deprecated: it remains the sanctioned way for a component that genuinely
// renders something to reach the editor (useExtensionComponent is built on it).
// What was deprecated is using it as a back door for behaviour-only components.
function TagButton (): JSX.Element {
    const [editor] = useLexicalComposerContext();

    // The payload is the mark ID. Deferred decision: a throwaway uid for now, so
    // each chip is at least distinct. When entity resolution lands, this becomes
    // the graph uid of the Person/Organization being mentioned --- MarkNode's
    // __ids then genuinely means "this range mentions these entities", and
    // $getMarkIDs answers that question directly. No signature change needed.
    return (
        <button
            type="button"
            onClick={ () => editor.dispatchCommand(INSERT_TAG_CHIP_COMMAND, crypto.randomUUID()) }>
            Insert #person chip
        </button>
    );
}

// No payload: the command searches for whatever is currently selected, and now
// says so in its type. The previous version passed `{ uid: "some-statement-uid",
// query: "some search query" }`, which the handler quietly ignored --- a payload
// type that described an intention nobody implemented.
function SearchButton (): JSX.Element {
    const [editor] = useLexicalComposerContext();
    const loading = useExtensionSignalValue(SearchInterviewExtension, "loading");

    return (
        <button
            type="button"
            disabled={ loading }
            onClick={ () => editor.dispatchCommand(PERFORM_SEARCH_COMMAND, undefined) }>
            { loading ? "Searching..." : "Search for selection" }
        </button>
    );
}

// ReactExtension renders `<>{contentEditable}{children}</>` by default, which
// would put our toolbar below the transcript. Overriding EditorChildrenComponent
// through `configExtension` is how the new model does layout composition --- the
// editor's chrome is configuration of an extension rather than JSX the route
// happens to nest in the right order.
function EditorChrome ({ contentEditable, children }: EditorChildrenComponentProps): JSX.Element {
    const Meter = useExtensionComponent(LatencyExtension);

    return (
        <>
            <TagMarkStyles />
            <div style={{ display: "flex", gap: "1rem", alignItems: "center", padding: "0.5rem 0" }}>
                <TagButton />
                <SearchButton />
                <Meter />
            </div>
            { contentEditable }
            { children }
        </>
    );
}

// -----------------------------------------------------------------------------
// TagChipPortals --- the ElementNode -> React bridge.
//
// A DecoratorNode gets React for free: the reconciler pulls JSX out of
// `decorate()` at the node's own position. TagChipNode extends MarkNode, hence
// ElementNode, so no such hook exists --- and it must stay an ElementNode,
// because its children are the tagged text (a DecoratorNode's getTextContent()
// returns "", which would silently erase those words from the statement text
// PersistenceExtension ships to the server).
//
// So we invert the flow and push instead: a mutation listener reports every
// chip's lifecycle, we resolve each one's unmanaged badge span, and React
// portals into it. The badge being `setDOMUnmanaged` is what makes this legal
// --- Lexical's mutation-attribution up-walk terminates there, so nothing React
// renders inside gets evicted as foreign DOM.
//
// Rendered via ReactExtension's `decorators` channel, which exists precisely for
// "JSX inside the editor context that is not location-dependent".
// -----------------------------------------------------------------------------
function TagChipPortals (): JSX.Element {
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
                        `:scope > .${ TAG_CHIP_BADGE_CLASS }`,
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

// Deliberately does not read the `data` signal. It reacts to SearchResultNode
// mutations, which is a strictly later event: `$applySearchResults` creates the
// nodes, the reconciler builds their badge spans, the mutation listener fires,
// and only then is there anything to portal into. Reading `data` here as well
// would close a loop --- create nodes -> mutation -> setHosts -> re-render ->
// create nodes --- and conflate "own the marks" with "paint inside the marks".
function SearchResultPortals (): JSX.Element {
    const [editor] = useLexicalComposerContext();

    // NodeKey -> the badge span to portal into. Held in React state (not a ref)
    // because adding or dropping an entry must trigger a re-render.
    const [hosts, setHosts] = useState<ReadonlyMap<NodeKey, HTMLElement>>(new Map());

    useEffect(
        () =>
            editor.registerMutationListener(SearchResultNode, mutations => {
                // Resolve a chip's portal target from its NodeKey. Returns null
                // when the node has no DOM yet (or no badge, e.g. a node
                // replacement swapped createDOM out from under us).
                const resolveHost = (key: NodeKey): HTMLElement | null =>
                    editor.getElementByKey(key)?.querySelector<HTMLElement>(
                        `:scope > .${ SEARCH_RESULT_BADGE_CLASS }`,
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
            { Array.from(hosts, ([key, host]) => createPortal(<SearchResult nodeKey={ key } />, host, key)) }
        </>
    );
}

// -----------------------------------------------------------------------------
// SearchDriver --- the React half of the search seam.
//
// Apollo's executor only exists inside a hook, so something has to be a component.
// But note what this component is not: it renders nothing, it takes no props, and
// the route neither knows it exists nor passes anything to it. It is registered
// through ReactExtension's `decorators` channel --- the same channel TagChipPortals
// and SearchResultPortals use --- which means "mount this inside the editor's React
// context". Search became a capability the editor has, rather than something the
// route configures it with, and the dependency arrow reversed accordingly.
//
// Reaching its own extension's output via `useExtensionDependency` is legal here
// for the same reason TagChipPortals may call `useLexicalComposerContext`:
// decorators render inside the composer, long after the extension graph is built.
//
// `useSignalValue` (not `useSignalEffect` from @preact/signals-react) is the right
// subscriber: it is `useSyncExternalStore`-based, so it needs no signals-react
// babel/swc transform and participates correctly in concurrent rendering. Both
// libraries do resolve to the single `@preact/signals-core` copy in node_modules,
// so a Lexical extension signal and a `playhead` signal are the same kind of thing.
// -----------------------------------------------------------------------------
function SearchDriver (): JSX.Element | null {
    const { query, data, loading } = useExtensionDependency(SearchInterviewExtension).output;

    // Subscribing to `query` is what turns a command dispatch into a re-render of
    // this component --- and nothing else in the editor re-renders, which is the
    // whole point of routing live data through signals instead of through props.
    const pendingQuery = useSignalValue(query);

    const [runSearch, searchState] = useLazyQuery(SEARCH_STATEMENTS_QUERY, {
        fetchPolicy: "network-only",
    });

    const onSearchUpdate = useEffectEvent(() => {
        loading.value = false;

        console.log("SearchDriver: onSearchUpdate fired with state", searchState);
        const { data: searchData } = searchState;

        if (searchState.error) {
            console.error("SearchDriver: search error", searchState.error);
        }
        if (searchData && !searchData.searchStatements) {
            console.warn("SearchDriver: onSearchUpdate fired with data but no searchStatements field", searchData);
        }
        if (searchData && searchData.searchStatements) {
            console.log("SearchDriver: onSearchUpdate fired with results", searchData.searchStatements);
            data.value = searchData;
            console.log("SearchDriver: data.value updated to", data.peek());
        }
    });

    useEffect(() => {
        async function queryHandler () {
            const loadingState = loading.peek();
            console.log({ loadingState, pendingQuery });

            if (pendingQuery !== null && !loadingState) {
                loading.value = true;
                try {
                    console.log("SearchDriver: queryHandler running with query", pendingQuery);
                    const res = await runSearch({
                        variables: {
                            fragment: pendingQuery,
                        },
                    });
                    console.log("SearchDriver: runSearch returned", res);
                    onSearchUpdate();
                } catch (error) {
                    console.warn("SearchDriver: runSearch error", error);
                }
            }
        }

        queryHandler();
    }, [pendingQuery]);

    return null;
}
