import {
    $createTextNode,
    $getNodeByKey,
    $getRoot,
    $getSelection,
    $isRangeSelection,
    $isTextNode,
    COMMAND_PRIORITY_LOW,
    defineExtension,
    safeCast,
    type NodeKey,
    type TextNode,
} from "lexical";
import { $dfs, $findMatchingParent, mergeRegister } from "@lexical/utils";
import { $unwrapMarkNode, $wrapSelectionInMarkNode, MarkExtension } from "@lexical/mark";
import { debounce } from "perfect-debounce";
import { StatementExtension } from "../statement/StatementExtension";
import { $isStatementNode, StatementNode } from "../statement/StatementNode";
import { INSERT_SEARCH_RESULT_COMMAND } from "./commands";
import { $createSearchResultNode, $isSearchResultNode, SEARCH_RESULT_BADGE_CLASS, SearchResultNode } from "./SearchResultNode";
import { findMatchRanges } from "./match-ranges";
import { createSearchOutput, SEARCH_TAG, type SearchOutput, type SearchOutputWithDispose, type SearchStatementsData } from "./search-output.svelte";
import { registerSvelteDecorator } from "../svelte-decorator";
import SearchResult from "./SearchResult.svelte";

export type { SearchOutput, SearchStatementsData };

// The query executor: config-injected, like persistence's editStatement/etc,
// not a client import inside this module. `null` disables search, a real
// mode read via `?.`, same pattern as persistence's null executors. Not
// reactive config, and could not be -- `build(editor, config)` runs exactly
// once, and there is no re-render for an extension to re-apply a later
// config value through (an extension is a value, not a component). This
// asymmetry with query/data/etc (which are reactive output, in
// search-output.svelte.ts) is intentional: the executor is a stable
// collaborator, the search state it drives is not.
export type SearchStatementsFn = (variables: {
    fragment: string;
    interviewUid: string;
}) => Promise<{ data?: SearchStatementsData }>;

export interface SearchInterviewConfig {
    searchStatements: SearchStatementsFn | null;
    interviewUid: string;
}

// -----------------------------------------------------------------------------
// SearchInterviewExtension --- the read path.
//
// Split across two files for a compiler reason, not a style preference:
// Svelte reserves the `$` prefix on bindings in any file matching
// /\.svelte\.[jt]s$/ (needed for `$state`/`$effect` to be legal there), and
// Lexical's own `$`-prefixed functions ("must run inside an active editor
// state") collide with that reservation. The reactive output object and its
// three effects live in ./search-output.svelte.ts; every `$`-prefixed
// Lexical function lives here, in a plain .ts file, and crosses that
// boundary through plain-named exports (`applySearchResults`, `seekToResult`)
// passed into `createSearchOutput` as callbacks -- the `.svelte.ts` file
// never needs a `$`-prefixed identifier of its own.
//
// Data flows one way, and every hop after the executor is a write to the
// shared reactive output:
//
//   the route/chrome writes `output.query`
//     -> the debounced executor (closure local in build, called via
//        output._runQuery) calls config.searchStatements
//     -> writes `output.data` / `output.loading`
//     -> the highlight-pass effect repaints marks and settles
//        `resultKeys` / `resultCount` / `focusedResult`
// -----------------------------------------------------------------------------

// Contract: given a result set (or `null` fragment, meaning "no search"),
// leave the document holding exactly the marks that set implies -- nothing
// stale from the previous search, nothing missing from this one.
//
// `fragment` is threaded in as a parameter rather than read off the live
// query: the query is the latest request, `results` is the response to some
// earlier one, and under a fast second search those are different strings.
// Highlighting a response with a query it did not answer is a stale-closure
// bug; passing both together makes them impossible to desynchronise.
//
// Returns the keys of every mark it painted, in document order -- the order
// is free rather than earned: root.getChildren() walks statements top to
// bottom, and $markMatchesInStatement marks occurrences left to right within
// one, so appending as we go is already document order.
//
// Exported plain-named (no $) so search-output.svelte.ts can call it -- the
// $-prefixed internal helpers it calls stay private to this module. Must run
// inside an active editor state (editor.read()/editor.update()) -- the $
// prefix that would normally say so cannot survive the crossing into a
// .svelte.ts file, so it is stated here in prose instead. The one call site
// (search-output.svelte.ts's highlight-pass effect) already wraps it in
// editor.update().
export function applySearchResults (results: SearchStatementsData | undefined, fragment: string | null): readonly NodeKey[] {
    const uids = new Set(results?.search.statementText.map(({ statement }) => statement.uid) ?? []);

    const root = $getRoot();
    const keys: NodeKey[] = [];

    for (const child of root.getChildren()) {
        if (!$isStatementNode(child)) {
            continue;
        }
        const statement = child;

        $clearSearchResults(statement);

        if (fragment !== null && uids.has(statement.getUid())) {
            keys.push(...$markMatchesInStatement(statement, fragment));
        }
    }

    return keys;
}

// Remove every SearchResultNode beneath `statement`, hoisting its children
// back into the parent, then heal the text runs the unwrap leaves behind.
// A traversal rather than a direct-grandchildren scan: inline marks sit at
// arbitrary depth among the text, not just as the statement's only child.
function $clearSearchResults (statement: StatementNode): void {
    // Collect before mutating: $dfs walks live node versions, and unwrapping
    // during the walk invalidates the cursor it is holding.
    const marks = $dfs(statement)
        .map(({ node }) => node)
        .filter($isSearchResultNode);

    for (const mark of marks) {
        $unwrapMarkNode(mark);
    }

    if (marks.length > 0) {
        $mergeAdjacentTextNodes(statement);
    }
}

// Unwrapping a mark hoists its TextNode children up beside their former
// siblings, so one logical run becomes several sibling TextNodes. Left
// unmerged, every search/clear cycle shatters the paragraph further, and the
// offsets findMatchRanges returns (relative to the statement's whole text)
// stop lining up with any single node. mergeWithSibling does the same quiet
// work splitText does in the other direction: it rebases any RangeSelection
// anchor/focus pointing into the absorbed node onto the survivor.
// isSimpleText is the guard -- false for TextNodes carrying format/style/mode,
// which merging would silently drop.
function $mergeAdjacentTextNodes (statement: StatementNode): void {
    let previous: TextNode | null = null;

    for (const child of statement.getChildren()) {
        if ($isTextNode(child) && child.isSimpleText()) {
            if (previous !== null) {
                previous = previous.mergeWithSibling(child);
                continue;
            }
            previous = child;
        } else {
            previous = null;
        }
    }
}

// Wrap each occurrence of `fragment` in `statement` in its own
// SearchResultNode. Offsets from findMatchRanges are relative to the
// statement's flattened text, but the text lives in one or more TextNodes and
// may be interrupted by TagChipNodes -- so the walk re-derives each child's
// span in flattened coordinates and intersects it with the ranges.
function $markMatchesInStatement (statement: StatementNode, fragment: string): readonly NodeKey[] {
    const ranges = findMatchRanges(statement.getTextContent(), fragment);
    if (ranges.length === 0) {
        return [];
    }

    const uid = statement.getUid();
    const keys: NodeKey[] = [];
    let offset = 0;

    for (const child of statement.getChildren()) {
        const size = child.getTextContentSize();
        const childStart = offset;
        const childEnd = offset + size;
        offset = childEnd;

        if (!$isTextNode(child) || !child.isSimpleText()) {
            continue;
        }

        // Ranges intersecting this child, clamped into child-local
        // coordinates and clipped to its bounds -- a range straddling a
        // TagChip boundary highlights the part that falls in this node.
        const local = ranges
            .filter(({ start, end }) => start < childEnd && end > childStart)
            .map(({ start, end }) => ({
                start: Math.max(start, childStart) - childStart,
                end: Math.min(end, childEnd) - childStart,
            }))
            .filter(({ start, end }) => end > start);

        if (local.length === 0) {
            continue;
        }

        // splitText takes cut points, not ranges. It remaps any
        // RangeSelection anchor/focus pointing into the original node onto
        // the correct piece with a rebased offset -- rebuilding by hand with
        // setTextContent would teleport the user's caret on every search.
        const cuts = [...new Set(local.flatMap(({ start, end }) => [start, end]))]
            .filter(cut => cut > 0 && cut < size)
            .sort((a, b) => a - b);

        const pieces = child.splitText(...cuts);

        // Walk the pieces alongside the same cut boundaries: a piece
        // starting at a range's start is a match, by construction.
        const starts = new Set(local.map(({ start }) => start));
        let pieceOffset = 0;

        for (const piece of pieces) {
            const pieceStart = pieceOffset;
            pieceOffset += piece.getTextContentSize();

            if (!starts.has(pieceStart)) {
                continue;
            }

            // insertBefore + append, not piece.replace(mark): replace's
            // selection remap for a point anchored on the replaced node
            // resolves to (mark, 0) while the mark is still empty (append
            // hasn't happened yet), so a caret inside a run that becomes a
            // match jumps to the mark's front. Reparenting sidesteps the
            // guess: piece is never destroyed, so no point anchored on it
            // ever needs relocating.
            const mark = $createSearchResultNode([uid]);
            piece.insertBefore(mark);
            mark.append(piece);
            keys.push(mark.getKey());
        }
    }

    return keys;
}

// Replace the text inside one mark, leaving the surrounding run intact.
// Unwrapping afterwards is what makes this a real edit rather than a
// re-highlight -- the mark described a match that no longer exists once the
// text changes.
//
// Exported plain-named (no $), even though it isn't called from
// search-output.svelte.ts today: step 6's chrome (SearchBar's
// replaceFocused/replaceAll) will call it directly. Must run inside an
// active editor state (editor.update()) -- the caller is responsible for
// that wrapper, same as every other plain-named export in this file.
export function replaceMark (mark: SearchResultNode, replacement: string): StatementNode | null {
    const statement = $findMatchingParent(mark, $isStatementNode);
    if (statement === null) {
        return null;
    }

    const children = mark.getChildren();
    const [first, ...rest] = children;

    if ($isTextNode(first)) {
        // setTextContent on the surviving child rather than a fresh TextNode,
        // so a selection anchored inside this node is rebased by Lexical
        // instead of left pointing at a node that no longer exists.
        first.setTextContent(replacement);
        for (const child of rest) {
            child.remove();
        }
    } else {
        // No text child to reuse (a mark containing only a TagChip, say).
        mark.append($createTextNode(replacement));
        for (const child of children) {
            child.remove();
        }
    }

    $unwrapMarkNode(mark);
    $mergeAdjacentTextNodes(statement);

    return statement;
}

export const SearchInterviewExtension = /* @__PURE__ */ defineExtension({
    nodes: () => [SearchResultNode],
    dependencies: [StatementExtension, MarkExtension],
    name: "@auohp/search-interview",
    config: /* @__PURE__ */ safeCast<SearchInterviewConfig>({
        searchStatements: null,
        interviewUid: "",
    }),

    // `build` receives `editor` as its first parameter, same as `register`
    // (verified against LexicalExtension.dev.mjs's actual call site --
    // `this.extension.build(editor, state.config, state.registerState)` --
    // not assumed from the type signature alone). That is what makes
    // constructing the real output here possible: createSearchOutput needs
    // `editor` for its effects' editor.update() calls, and needs
    // `config.searchStatements` for the debounced query executor, both of
    // which `build` already has. This matches PersistenceExtension and
    // StatementExtension's shape (both also do real work in `build`, not
    // just a passthrough) rather than diverging from precedent the way an
    // earlier draft of this file briefly did.
    build (editor, config): SearchOutputWithDispose {
        // Taken directly from this extension's own config, not read off
        // PersistenceExtension's output -- both extensions take their own
        // copy of the same `interviewUid`, wired from the single call site in
        // editor.ts, rather than search structurally depending on persistence
        // to learn which interview it's searching. The two copies can't
        // disagree because there is exactly one place that constructs them.
        const { interviewUid } = config;

        // `output` is assigned below, once `createSearchOutput` returns.
        // `debouncedQueryHandler` and the `seekToResult` callback passed into
        // `createSearchOutput` both close over it before that assignment
        // happens -- safe because a closure captures the variable, not its
        // value at definition time, and neither is actually called until
        // well after `build` returns: `debouncedQueryHandler` defers by
        // definition, and the effects that call `seekToResult`/`runQuery`
        // inside `createSearchOutput` do not run their bodies synchronously
        // either -- per the Svelte docs, effects run "in a microtask after
        // state changes", not inline when `$effect(...)` is called. Ordinary
        // forward reference, not a race.
        let output: SearchOutputWithDispose;

        const debouncedQueryHandler = debounce(async (fragment: string) => {
            if (fragment === "") {
                return;
            }
            output.loading = true;
            try {
                const result = await config.searchStatements?.({ fragment: `"${ fragment }"`, interviewUid });
                // Only `data` is set here. `resultCount`/`focusedResult` are
                // settled by the highlight pass, which is downstream of this
                // write and counts marks actually painted -- not matching
                // statements, which a repeated-phrase statement would
                // undercount.
                if (result?.data) {
                    output.data = result.data;
                }
            } finally {
                output.loading = false;
            }
        }, 1_500, { leading: false, trailing: true });

        output = createSearchOutput({
            editor,
            runQuery: fragment => debouncedQueryHandler(fragment),
            cancelQuery: () => debouncedQueryHandler.cancel(),
            applySearchResults,
            seekToResult: key => {
                editor.update(
                    () => {
                        const mark = $getNodeByKey(key) ?? undefined;
                        if (!$isSearchResultNode(mark)) {
                            return;
                        }

                        // Select the matched run rather than collapsing to a
                        // caret at its edge -- what Cmd-G does in most
                        // editors, and it leaves the document one keystroke
                        // from replacing the match.
                        mark.select(0, mark.getChildrenSize());
                    },
                    // The caret move is our write, not the human's. Untagged
                    // it would reach the re-search listener below; that
                    // listener happens to ignore it (a selection change
                    // dirties no leaves), but relying on that coincidence is
                    // how the loop comes back the next time the gate changes.
                    { tag: ["history-merge", SEARCH_TAG] },
                );
            },
        });

        return output;
    },

    register (editor, _config, state) {
        const output = state.getOutput();

        const unregister = mergeRegister(
            editor.registerCommand(
                INSERT_SEARCH_RESULT_COMMAND,
                id => {
                    const selection = $getSelection();
                    if (!$isRangeSelection(selection)) {
                        return false;
                    }
                    $wrapSelectionInMarkNode(selection, false, id, ids => $createSearchResultNode(ids));
                    return true;
                },
                COMMAND_PRIORITY_LOW,
            ),

            // The mark's Svelte face, mounted into the unmanaged badge span
            // SearchResultNode.createDOM builds. `props` hands the component
            // the shared reactive `output` object (not a derived `focused`
            // boolean) precisely because registerSvelteDecorator's `props`
            // callback runs once, at mount -- a plain boolean would never
            // update. SearchResult.svelte derives `focused` itself with
            // $derived off `output.resultKeys`/`output.focusedResult`, which
            // stays live because `output` is the same $state proxy
            // throughout, not a snapshot taken at props-build time.
            registerSvelteDecorator(editor, SearchResultNode, {
                component: SearchResult,
                props: key => ({ output, nodeKey: key }),
                resolveHost: element =>
                    element.querySelector<HTMLElement>(`:scope > .${ SEARCH_RESULT_BADGE_CLASS }`),
            }),

            // Re-run the search when the human edits the transcript, so the
            // result set and its highlights stay honest about the text
            // actually on screen. This cannot be a mutation listener on
            // SearchResultNode: mutations do not bubble (typing inside a
            // highlighted run mutates the mark's TextNode child, not the
            // mark; typing anywhere else mutates no mark at all -- the one
            // class guaranteed not to see ordinary edits is the one wrapping
            // the matches), and applySearchResults destroys/recreates every
            // mark on each result set, so a listener reacting to mark
            // mutations would feed itself forever.
            editor.registerUpdateListener(({ tags, dirtyLeaves }) => {
                // Cheapest predicate first, and the most decisive: with no
                // query there is no result set to keep honest. Must not fall
                // through to the debounced handler with an empty string --
                // it no-ops on "", but only after the debounce has already
                // scheduled a timer, displacing a pending real search. A
                // plain read: this is a Lexical update-listener callback, not
                // an $effect, so there is nothing here for Svelte to track in
                // the first place -- unlike the untrack() sites in
                // search-output.svelte.ts, this one needs no wrapper because
                // it was never inside a tracked context to begin with. Former
                // peek site #3 in the source ("query.peek() in the re-search
                // listener").
                const pending = output.query;
                if (!pending) {
                    return;
                }

                // Our own highlight pass, and the initial document seed, are
                // not the human changing the transcript. Reacting to
                // SEARCH_TAG in particular is the infinite loop described
                // above.
                if (tags.has(SEARCH_TAG) || tags.has("history-merge")) {
                    return;
                }

                // Selection-only commits (caret moves, clicks, focus changes)
                // arrive constantly and dirty nothing. dirtyLeaves is the
                // honest signal for "text actually changed"; dirtyElements is
                // not, since it always contains root (every commit
                // reconciles from the top).
                if (dirtyLeaves.size === 0) {
                    return;
                }

                // Deliberately unscoped: any text edit anywhere re-runs the
                // search. The narrower "did this edit touch a mark" test
                // cannot see the two cases that matter most -- growing a
                // match from adjacent text, and typing a brand-new match into
                // a statement that never held one. The trailing debounce
                // already collapses a burst of keystrokes into one round-trip.
                // `_runQuery` is the same debounced executor `build`
                // constructed, reached off the output object rather than a
                // shared closure -- see SearchOutputWithDispose for why.
                output._runQuery(pending);
            }),
        );

        return () => {
            output._dispose();
            unregister();
        };
    },
});
