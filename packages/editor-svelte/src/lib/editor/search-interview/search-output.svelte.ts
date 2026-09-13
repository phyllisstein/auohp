import { untrack } from "svelte";
import type { LexicalEditor, NodeKey } from "lexical";
import type { SearchStatementsQuery } from "./__generated__/queries.gql";

// -----------------------------------------------------------------------------
// The $state factory and its three effects for SearchInterviewExtension.
//
// Split out of the extension module for a compiler reason, not a style
// preference: Svelte reserves the `$` prefix on bindings (imports and
// declarations -- member access, object keys and strings are unaffected) in
// any file matching /\.svelte\.[jt]s$/, which is what makes `$state`/`$effect`
// legal there in the first place. Lexical's own naming convention uses a `$`
// prefix for the opposite reason -- it means "must run inside an active
// editor state" -- so a single file cannot hold both `$state`/`$effect` and
// Lexical's `$`-prefixed functions. `$effect`/`$effect.root` themselves are
// `$`-prefixed, so it is not enough to keep them out of the extension file;
// they have to live HERE, with the state, while the Lexical logic they invoke
// stays in SearchInterviewExtension.ts and crosses this boundary through
// plain-named exports (`applySearchResults`, no `$`).
//
// Renaming Lexical's imports at the point of use (`import { $getRoot as
// getRoot }`) also compiles, and was considered and rejected: the `$` prefix
// is load-bearing documentation of which functions need an active editor
// state, and stripping it to dodge a compiler rule would lose that signal
// silently at every call site it touches.
// -----------------------------------------------------------------------------

// Result shape: the generated operation type, not a hand-written structural
// one. Unlike persistence's executor types (PLAN.md sec 3.4: keep the write
// path independent of the GraphQL library), this extension owns the query --
// codegen's near-operation-file preset generates
// search-interview/__generated__/queries.gql.ts right beside queries.ts,
// which is the feature-scoped ownership PLAN.md sec 1 wants. The executor
// itself stays config-injected (SearchStatementsFn in
// SearchInterviewExtension.ts), same as persistence's executors -- only the
// result SHAPE is generated, not the call mechanism.
export type SearchStatementsData = SearchStatementsQuery;

// Output shape: a $state object, not a namedSignals bundle. Every field here
// earns reactivity -- each takes a second value mid-session and something
// reacts to it -- so unlike PersistenceOutput there is no inert seam to drop,
// and unlike LatencyOutput there is no single-field simplicity either; this
// is the all-reactive case. Precedent for storing it as build()'s return:
// `@lexical/extension` stores `build()`'s return BY REFERENCE
// (LexicalExtension.dev.mjs, `output = extension.build(...)`, `getOutput: ()
// => output`), never spread or copied -- so the proxy identity survives the
// module boundary (this file to SearchInterviewExtension.ts) exactly as it
// survives the extension boundary, and a reader off `getOutput()` tracks
// correctly regardless of which file constructed the object.
export interface SearchOutput {
    /** The pending search string. `null` means idle -- no search requested yet. */
    query: string | null;
    /** Latest results, or `undefined` before the first response. */
    data: SearchStatementsData | undefined;
    /** Whether a search is currently in flight. */
    loading: boolean;
    /** The index of the result that has the caret, or `null` if none. */
    focusedResult: number | null;
    /** The total number of results, or `0` if none. */
    resultCount: number;
    /**
     * The marks painted by the last highlight pass, in document order.
     *
     * This is the authority for what "result N" means: `resultCount` counts
     * matching STATEMENTS, but a statement saying "ACT UP ... ACT UP" carries
     * two marks, so the two would disagree about the total if either stood in
     * for "which result". Written by the highlight pass below, the one
     * moment when the marks and their order are both known for certain.
     */
    resultKeys: readonly NodeKey[];
    /** The replacement string. Empty is legal -- it means "delete the match". */
    replacement: string;
}

// SEARCH_TAG lives here too: both effects that tag an editor.update() call
// need it, and it has no $-prefix issue (a plain string constant).
export const SEARCH_TAG = "auohp-search-highlight";

export interface SearchOutputDeps {
    editor: LexicalEditor;
    /** Runs the debounced query; called with the trimmed fragment string. */
    runQuery: (fragment: string) => void;
    /** Cancels any in-flight debounced query, called on teardown. */
    cancelQuery: () => void;
    /** Lexical logic that must run inside an active editor state -- see
     * SearchInterviewExtension.ts. Passed in rather than imported so this
     * file never needs a `$`-prefixed identifier of its own. */
    applySearchResults: (results: SearchStatementsData | undefined, fragment: string | null) => readonly NodeKey[];
    seekToResult: (key: NodeKey) => void;
}

// The extension's actual output type is SearchOutput; this adds two fields
// for the hop between build() (which constructs everything, including the
// debounced query executor, and cannot return a teardown of its own -- only
// register()/afterRegistration() can) and register() (which needs to call
// both the debounced executor, for the re-search-on-edit listener, and the
// disposer, in its own teardown). Not exposed on SearchOutput itself: this is
// plumbing between this extension's own two lifecycle methods, not part of
// what a component or another extension should read or call.
export interface SearchOutputWithDispose extends SearchOutput {
    _dispose: () => void;
    _runQuery: (fragment: string) => void;
}

// Builds the $state object and wires its three effects. Called once from
// SearchInterviewExtension's `build`, which -- verified against the actual
// `@lexical/extension` source, not assumed from the type signature -- does
// receive `editor` as its first parameter, same as `register`. That is what
// makes constructing the real output here (rather than in `register`)
// possible at all, matching PersistenceExtension/StatementExtension's shape
// of doing real work in `build`. The `$effect.root` disposer travels back to
// `build`'s caller riding on the output object itself (`_dispose`, see
// SearchOutputWithDispose) because `build` cannot return a teardown of its
// own -- only `register`/`afterRegistration` can -- so `register` reads
// `_dispose` off `state.getOutput()` and calls it in its own teardown.
export function createSearchOutput (deps: SearchOutputDeps): SearchOutputWithDispose {
    const { editor, runQuery, cancelQuery, applySearchResults, seekToResult } = deps;

    const output = $state<SearchOutput>({
        query: null,
        data: undefined,
        loading: false,
        focusedResult: null,
        resultCount: 0,
        resultKeys: [],
        replacement: "",
    });

    // Preact's `subscribe` invoked its callback immediately with the current
    // value; at registration that value is `undefined` and the document is
    // not even seeded yet ($initialEditorState runs after every register), so
    // the first call was noise. $effect has the same immediate-first-run
    // behaviour (confirmed against the Svelte docs, not assumed), so the
    // guard is still needed. Swallowing it explicitly beats guarding on
    // `results === undefined` inside the effect, because `data` legitimately
    // returns to `undefined` later and that case must still clear the marks.
    let primed = false;

    // $effect.root gives these effects an owner outside any component's init
    // phase (register() is not one) and returns a disposer.
    const dispose = $effect.root(() => {
        // The query-change effect. Fires only when `output.query` changes --
        // a NEW query, for which discarding the old focused position is
        // right. The re-search-on-edit listener in SearchInterviewExtension.ts
        // deliberately calls `runQuery` directly instead of writing
        // `output.query`, precisely so it does not land here and reset a
        // position the user is standing on. Two reasons this matters under
        // runes, where the source only had one: routing that path through
        // this effect would look like a simplification and silently
        // reintroduce the jump-to-first-hit bug (the source's problem); AND,
        // unlike React's explicit dependency array, `$effect` tracks every
        // reactive read automatically -- writing `output.query` from the
        // update listener would trigger THIS effect, which writes three more
        // fields, which may re-trigger the highlight effect below. The
        // source's bug was a UX regression; this one would be a cascade.
        $effect(() => {
            const pendingQuery = output.query;

            output.focusedResult = null;
            output.resultCount = 0;
            output.resultKeys = [];

            if (!pendingQuery) {
                output.data = undefined;
                output.loading = false;
                return;
            }
            // `loading = true` is not set here -- the debounced executor
            // (SearchInterviewExtension.ts) owns it exclusively, since it's
            // the only writer that runs on every path that can trigger a
            // search, not just this one. The re-search-on-edit listener
            // bypasses this effect entirely (calls output._runQuery
            // directly, deliberately -- see the comment above), so a second
            // writer here would go true immediately on this path but only
            // once the debounce fires on that one -- two different meanings
            // for one flag depending on which path triggered the search, and
            // two overlapping searches racing to clear it early. One owner,
            // request-lifecycle-only.
            runQuery(pendingQuery);
        });

        // The highlight pass: repaints marks whenever `output.data` changes,
        // settles resultKeys/resultCount/focusedResult.
        //
        // Timing: the source's data.subscribe(...) fired synchronously;
        // $effect batches to a microtask. Unobservable here -- the write that
        // triggers this effect happens on completion of a debounced (1500ms)
        // network round trip, already many frames past any user action, so a
        // microtask on top changes nothing a person could perceive. The one
        // place synchrony could matter is the other direction (this effect's
        // own editor.update() call, via applySearchResults/seekToResult), and
        // Lexical already defers ITS commit to a microtask regardless (see
        // commit C's afterRegistration reasoning) -- so this adds a microtask
        // ahead of a mechanism that already runs on one.
        $effect(() => {
            // Track output.data; nothing else read here should subscribe.
            const results = output.data;

            if (!primed) {
                primed = true;
                return;
            }

            // `untrack()` around both reads below is load-bearing, not
            // stylistic. `$effect` has no dependency array -- it tracks every
            // reactive read that happens during the SYNCHRONOUS extent of its
            // run, including ones nested inside a callback passed to
            // `editor.update()` (that callback runs synchronously; had it
            // been deferred to a microtask instead, the same code would be
            // UNtracked, and this would be a stale-read bug rather than a
            // self-trigger one -- tracking follows the callee's scheduling,
            // not lexical nesting). Being inside a Lexical callback does not
            // exempt a read from Svelte's tracking; only `untrack()` does.
            // Without it, this effect (which already tracks `output.data`,
            // above) would also track `output.query` and
            // `output.focusedResult` -- and since this same effect writes
            // `output.focusedResult` a few lines down, an unwrapped read here
            // would make the effect re-trigger itself on its own write.
            // Former peek sites #1 and #2 in the source (`query.peek()` in
            // the data subscriber, `focusedResult.peek()` in the clamp) map
            // onto exactly these two `untrack()` calls -- both were "read the
            // current value without subscribing", which is what `untrack()`
            // gives a tracked context that `.peek()` gave a signal.
            //
            // `history-merge` is load-bearing, not decoration: wrapping text
            // in a MarkNode leaves getTextContent() byte-identical, so
            // PersistenceExtension would otherwise fire an editStatement per
            // highlighted statement, saving text that never changed.
            // SEARCH_TAG rides alongside it: history-merge says "not a user
            // edit" (to persistence/undo), SEARCH_TAG says who wrote it, so
            // the re-search listener declines to react to its own repaint.
            // The two claims are orthogonal and both are needed.
            editor.update(
                () => {
                    const keys = applySearchResults(results, untrack(() => output.query));

                    output.resultKeys = keys;
                    output.resultCount = keys.length;

                    // Clamp rather than reset. A re-search triggered by
                    // editing the transcript must not throw the user back to
                    // the first hit -- they are typically standing on the hit
                    // they just edited. Fall back to 0 only when there was no
                    // position to keep, to null when nothing matched.
                    const focused = untrack(() => output.focusedResult);
                    output.focusedResult = keys.length === 0
                        ? null
                        : Math.min(focused ?? 0, keys.length - 1);
                },
                { tag: ["history-merge", SEARCH_TAG] },
            );
        });

        // Move the caret to the focused result. Must track
        // output.focusedResult only -- output.resultKeys is read inside
        // `untrack()` (former peek site #4, "resultKeys.peek() in the
        // focused-result seek"). Without the wrapper this effect would also
        // re-run on every repaint (resultKeys changes on every search),
        // firing a caret move nobody asked for. Re-reading the array fresh,
        // rather than closing over a value captured above, still matters on
        // its own terms too: a debounced re-search may have repainted every
        // mark between the focus change and this callback running.
        $effect(() => {
            const index = output.focusedResult;
            if (index === null) {
                return;
            }

            const key = untrack(() => output.resultKeys)[index];
            if (key === undefined) {
                return;
            }

            seekToResult(key);
        });

        return () => {
            cancelQuery();
        };
    });

    // `_dispose` and `_runQuery` ride on the output object itself rather than
    // being returned alongside it -- see the module doc above for why
    // build() needs this hop (register() reads both off state.getOutput()).
    // Assigning them here reuses the $state proxy `output` already is;
    // nothing reads either reactively, so their presence on the proxy is
    // inert beyond being a place to park two function references.
    return Object.assign(output, { _dispose: dispose, _runQuery: runQuery });
}
