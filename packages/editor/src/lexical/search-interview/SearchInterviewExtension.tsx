import {
    $createTextNode,
    $getNodeByKey,
    $getRoot,
    $getSelection,
    $isRangeSelection,
    $isTextNode,
    COMMAND_PRIORITY_LOW,
    configExtension,
    defineExtension,
    safeCast,
    type NodeKey,
    type TextNode,
} from "lexical";
import { namedSignals, type Signal } from "@lexical/extension";
import { $unwrapMarkNode, $wrapSelectionInMarkNode, MarkExtension } from "@lexical/mark";
import { ReactExtension } from "@lexical/react/ReactExtension";
import { useExtensionSignalValue } from "@lexical/react/useExtensionSignalValue";
import { $dfs, $findMatchingParent, mergeRegister } from "@lexical/utils";
import { debounce } from "perfect-debounce";
import type { JSX } from "react";
import type { SearchStatementsQuery, SearchStatementsQueryVariables } from "~/__generated__/queries.gql";
import { StatementExtension } from "~/lexical/statement/StatementExtension";
import { $isStatementNode, type StatementNode } from "~/lexical/statement/StatementNode";
import { useNodeDecorators, type ResolveHost } from "~/lexical/react-decorator";
import { INSERT_SEARCH_RESULT_COMMAND } from "./commands";
import { findMatchRanges } from "./match-ranges";
import { SearchResult, SearchResultStyles } from "./SearchResult";
import { $createSearchResultNode, $isSearchResultNode, SEARCH_RESULT_BADGE_CLASS, SearchResultNode } from "./SearchResultNode";


export type SearchStatementsData = SearchStatementsQuery | undefined;

// The query executor: config-injected, like PersistenceExtension's
// `editStatement` and friends. The route closes it over its Apollo client, so
// this module never touches a GraphQL client and the extension stays testable
// against a stub. `null` disables search --- a real mode, read via `?.`.
export type SearchStatementsFn = (variables: SearchStatementsQueryVariables) => Promise<{ data?: SearchStatementsQuery | undefined }>;

export interface SearchInterviewConfig {
    searchStatements: SearchStatementsFn | null;
    /** The interview to scope results to. Taken directly, not read off PersistenceExtension. */
    interviewUid: string;
}

// -----------------------------------------------------------------------------
// SearchInterviewExtension --- the read path, and a worked example of the one
// mechanism that carries live data into an extension.
//
// Config is the transport for things that never change for the editor's
// lifetime --- the executor, the interview uid. Search state is the opposite:
//
//   `LexicalBuilder` calls `build(editor, config)` exactly once, at editor
//   construction. There is no later re-apply pass, because there is no
//   re-render for an extension --- an extension is a value, not a component.
//   So anything that takes a second value mid-session lives in a signal on the
//   output, and config is at most that signal's seed.
//
// Trying to push live data through config instead poisons the editor's
// lifetime. The route must `useMemo` the extension, and
// `LexicalExtensionComposer` memoises the editor on that extension's identity
// and disposes the old one --- so putting a per-render Apollo result in the dep
// array rebuilds the editor mid-search and throws away the user's unsaved edits
// and caret.
//
// Data flows one way, and every hop after the executor is a signal write:
//
//   SearchBar writes `query`
//     -> the `query` subscriber resets position and calls the debounced executor
//     -> the executor calls `searchStatements`, writes `data` / `loading`
//     -> the `data` subscriber repaints marks and settles
//        `resultKeys` / `resultCount` / `focusedResult`
//     -> the `focusedResult` subscriber selects the focused mark
//
// All of it lives in `register`. It used to be split with a render-nothing
// SearchDriver component, because Apollo's executor only existed inside a hook;
// with the executor injected, nothing here needs React, and the debounce timer
// now dies with the editor instead of outliving it.
// -----------------------------------------------------------------------------
// Unlike PersistenceOutput, every field here earns its Signal: each one takes a
// second value mid-session and something reacts to it. `subscribe` callbacks run
// untracked, so reading a sibling signal inside one with `.peek()` or `.value`
// never cross-subscribes; `.peek()` is used anyway, to say so.
export interface SearchOutput {
    /** The pending search string. `null` means idle --- no search requested yet. */
    query: Signal<string | null>;
    /** Latest results, or `undefined` before the first response. */
    data: Signal<SearchStatementsData>;
    /** Whether a search request is currently in flight. */
    loading: Signal<boolean>;
    /** The index of the result that has the caret, or `null` if none. */
    focusedResult: Signal<number | null>;
    /** The total number of results, or `0` if none. */
    resultCount: Signal<number>;
    /**
     * The marks painted by the last highlight pass, in document order.
     *
     * This is the authority for what "result N" means. A statement saying
     * "ACT UP ... ACT UP" is one hit to the server and two marks on the page,
     * and the decorator seam's slots arrive in mutation order, not document
     * order --- neither can stand in for it.
     *
     * Written by `$applySearchResults`, which is the one moment when the marks
     * and their order are both known for certain.
     */
    resultKeys: Signal<readonly NodeKey[]>;
    /** The replacement string. Empty is legal --- it means "delete the match". */
    replacement: Signal<string>;
}

// Stamped on the `editor.update()` that paints search highlights, so listeners
// can tell the search's own writes apart from a human's typing. Without it the
// re-search-on-edit listener in `register` would react to the repaint it just
// caused and spin forever --- the marks it watches are destroyed and recreated
// on every result set.
const SEARCH_TAG = "auohp-search-highlight";

// Contract: given a result set (or `undefined`, meaning "no search"), leave the
// document holding exactly the marks that set implies --- nothing stale from the
// previous search, nothing missing from this one.
//
// The marks are now INLINE. Previously this wrapped every child of a matching
// StatementNode in a single SearchResultNode, so the mark was a container for
// the whole paragraph and the highlight was a full-width band behind it. Now a
// statement gets one mark per literal occurrence of the query, wrapping only the
// matched run:
//
//   before: statement -> SearchResultNode -> [all children]
//   after:  statement -> [Text("We shut "), Mark -> [Text("ACT UP")], Text(" down")]
//
// `fragment` is threaded in as a parameter rather than read from the `query`
// signal inside. The signal is the LATEST request; `results` is the response to
// some earlier one, and under a fast second search those are different strings.
// Highlighting a response with a query it did not answer is the classic
// stale-closure bug, and passing both together makes them impossible to
// desynchronise.
// Returns the keys of every mark it painted, in document order --- the caller
// stores this as `resultKeys`, which is what makes "jump to result N" and the
// "N of M" counter agree with each other and with the page.
//
// The order is free rather than earned: `root.getChildren()` walks statements
// top to bottom, and `$markMatchesInStatement` marks occurrences left to right
// within one, so appending as we go is already document order. Recovering it
// afterwards would mean a second full traversal.
function $applySearchResults (results: SearchStatementsData, fragment: string | null): readonly NodeKey[] {
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


// Remove every SearchResultNode beneath `statement`, hoisting its children back
// into the parent, then heal the text runs the unwrap leaves behind.
//
// The old version only looked at direct grandchildren, which was sufficient when
// a mark WAS the statement's only child. Inline marks sit at arbitrary depth
// among the text, so the search has to be a traversal.
function $clearSearchResults (statement: StatementNode): void {
    // Collect before mutating: `$dfs` walks live node versions, and unwrapping
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
// siblings, so `[Text("We shut "), Mark[Text("ACT UP")], Text(" down")]` becomes
// three sibling TextNodes where the document logically has one run. Left
// unmerged, every search/clear cycle shatters the paragraph further, and the
// offsets `findMatchRanges` returns (which are relative to the statement's whole
// text) stop lining up with any single node.
//
// `mergeWithSibling` is the counterweight, and it does the same quiet work
// `splitText` does in the other direction: it rebases any RangeSelection
// anchor/focus pointing into the absorbed node onto the survivor. `isSimpleText`
// is the guard --- it is false for TextNodes carrying format/style/mode, and
// merging those would silently drop the formatting of one side.
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


// Wrap each occurrence of `fragment` in `statement` in its own SearchResultNode.
//
// Offsets from `findMatchRanges` are relative to the statement's FLATTENED text
// (`statement.getTextContent()`), but the text lives in one or more TextNodes and
// may be interrupted by TagChipNodes. So the walk below re-derives each child's
// span in flattened coordinates and intersects it with the ranges --- which is
// also why ranges are processed per-child rather than per-range.
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

        // Ranges intersecting this child, clamped into child-local coordinates
        // and clipped to its bounds --- a range straddling a TagChip boundary
        // highlights the part that falls in this node and is dropped elsewhere.
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

        // `splitText` takes cut points, not ranges: the flattened, deduped,
        // in-bounds boundaries. It returns the resulting nodes left-to-right, and
        // --- the part worth internalising --- it remaps any RangeSelection
        // anchor/focus that pointed into the original node onto the correct piece
        // with a rebased offset. Rebuilding the run by hand with setTextContent
        // would teleport the user's caret on every search.
        const cuts = [...new Set(local.flatMap(({ start, end }) => [start, end]))]
            .filter(cut => cut > 0 && cut < size)
            .sort((a, b) => a - b);

        const pieces = child.splitText(...cuts);

        // Walk the pieces alongside the same cut boundaries to decide which are
        // matches. A piece starting at a range's start is a match; the boundary
        // list and the piece list are in lockstep by construction.
        const starts = new Set(local.map(({ start }) => start));
        let pieceOffset = 0;

        for (const piece of pieces) {
            const pieceStart = pieceOffset;
            pieceOffset += piece.getTextContentSize();

            if (!starts.has(pieceStart)) {
                continue;
            }

            // One mark per occurrence, all carrying the statement's uid, so the
            // badge portal and any future "jump to hit N" affordance can still
            // resolve back to the statement that matched.
            //
            // `insertBefore` + `append`, NOT `piece.replace(mark)`, and the
            // difference is the user's caret. `replace` is selection-aware, but
            // its remap for a point anchored on the replaced node is
            // `$moveSelectionPointToEnd(anchor, mark)` --- and at that instant
            // the mark is still EMPTY, because `piece` has not been appended
            // yet. "End of an empty element" resolves to the element-anchored
            // point (mark, 0); the subsequent append re-homes the text but
            // nothing re-derives the selection, so a caret sitting inside a run
            // that becomes a match jumps to the front of its new mark. (Type
            // "GMaichC" back to "GMHC" over a live search and watch it happen.)
            //
            // Reparenting sidesteps the guess entirely: `piece` is never
            // destroyed, so no point anchored on it ever needs relocating.
            // Same principle as preferring `splitText` to `setTextContent`
            // above --- move nodes, never recreate the ones selection names.
            const mark = $createSearchResultNode([uid]);
            piece.insertBefore(mark);
            mark.append(piece);
            keys.push(mark.getKey());
        }
    }

    return keys;
}


// Replace the text inside one mark, leaving the surrounding run intact.
//
// The mark is an ElementNode wrapping one or more TextNodes, so "replace the
// match" means: put `replacement` into the first child, drop the rest, then
// unwrap. Unwrapping is what makes this a real edit rather than a re-highlight
// --- the mark described a match that no longer exists once the text changes,
// and leaving it would strand a highlight around text that does not match.
//
// `$mergeAdjacentTextNodes` afterwards is the same healing step $clearSearchResults
// performs, and for the same reason: the hoisted children arrive as siblings of
// the runs they were split out of, and an unmerged paragraph shatters further
// with every replace.
//
// Returns the containing statement so callers can report what changed.
export function $replaceMark (mark: SearchResultNode, replacement: string): StatementNode | null {
    // `$findMatchingParent` already narrows to StatementNode via its type-guard
    // overload, so this is a null check rather than a second type test --- the
    // guard itself will not accept a nullable argument.
    const statement = $findMatchingParent(mark, $isStatementNode);
    if (statement === null) {
        return null;
    }

    const children = mark.getChildren();
    const [first, ...rest] = children;

    if ($isTextNode(first)) {
        // setTextContent on the surviving child, rather than building a fresh
        // TextNode, so that a selection anchored inside this node is rebased by
        // Lexical instead of being left pointing at a node that no longer exists.
        first.setTextContent(replacement);
        for (const child of rest) {
            child.remove();
        }
    } else {
        // No text child to reuse (a mark containing only a TagChip, say). Insert
        // the replacement as a new node ahead of whatever is there and clear the
        // rest, which keeps the branch total rather than silently doing nothing.
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
    config: /* @__PURE__ */ safeCast<SearchInterviewConfig>({
        searchStatements: null,
        interviewUid: "",
    }),
    dependencies: [
        StatementExtension,
        MarkExtension,
        configExtension(ReactExtension, { decorators: [SearchResultPortals] }),
    ],
    name: "@auohp/search-interview",

    // The return type is annotated rather than inferred, and that is load-bearing
    // for a reason that has nothing to do with documentation: `dependencies` above
    // names `SearchResultPortals`, and its body asks for this extension's output.
    // Left to inference that is a cycle TypeScript refuses to resolve. Annotating
    // here (and annotating SearchResultPortals' return type) cuts it in both
    // directions --- neither side needs the other's body to compute its type.
    build: (): SearchOutput => namedSignals({
        query: null as string | null,
        data: undefined as SearchStatementsData,
        loading: false,
        focusedResult: null as number | null,
        resultCount: 0,
        resultKeys: [] as readonly NodeKey[],
        replacement: "",
    }),

    register (editor, config, state) {
        const { query, data, loading, focusedResult, resultCount, resultKeys } = state.getOutput();
        const { searchStatements, interviewUid } = config;

        // The one owner of `loading`, because it is the one writer on every path
        // that can start a request --- a new query and a re-search-on-edit alike.
        // `loading` therefore means "a request is in flight", not "a search is
        // pending", and two overlapping requests cannot clear each other's flag
        // early from two different places.
        //
        // Last request wins. A response that arrives after a newer request was
        // issued, or after the query was cleared, answers a question nobody is
        // asking any more; writing it would repaint stale highlights.
        let latestRequest = 0;
        const runQuery = debounce(async (fragment: string) => {
            if (fragment === "" || !searchStatements) {
                return;
            }

            const request = ++latestRequest;
            loading.value = true;
            try {
                const result = await searchStatements({ fragment: `"${ fragment }"`, interviewUid });
                if (request !== latestRequest || query.peek() === null) {
                    return;
                }

                // Only `data` is set here. `resultCount`/`focusedResult` are
                // settled by the highlight pass downstream, which counts the
                // marks it painted rather than the statements that matched.
                if (result.data) {
                    data.value = result.data;
                }
            } catch (error) {
                console.error("SearchInterviewExtension: search failed", error);
            } finally {
                if (request === latestRequest) {
                    loading.value = false;
                }
            }
        }, 1_500, { leading: false, trailing: true });

        // Preact's `subscribe` invokes its callback immediately with the current
        // value. At registration that value is `undefined` and the document is not
        // seeded yet --- $initialEditorState's commit lands a microtask after
        // registration --- so the first call is noise. Swallowing it explicitly
        // beats guarding on `results === undefined` inside the subscriber, because
        // `data` legitimately returns to `undefined` later and that case must still
        // clear the marks.
        let primed = false;

        return mergeRegister(
            () => {
                runQuery.cancel();
                latestRequest++;
            },

            // A NEW query, for which discarding the old position is right. The
            // re-search-on-edit listener below deliberately calls `runQuery`
            // directly instead of writing `query.value`, precisely so it does not
            // land here and reset a position the user is standing on. Routing that
            // path through this signal would look like a simplification and would
            // silently reintroduce the jump-to-first-hit bug.
            query.subscribe(pending => {
                focusedResult.value = null;
                resultCount.value = 0;
                resultKeys.value = [];

                if (!pending) {
                    runQuery.cancel();
                    latestRequest++;
                    data.value = undefined;
                    loading.value = false;
                    return;
                }

                runQuery(pending);
            }),

            // The highlight pass.
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
                //
                // SEARCH_TAG rides alongside: `history-merge` says "this is not a
                // user edit" (to persistence and undo), while SEARCH_TAG says who
                // wrote it, so the re-search listener can decline to react to its
                // own highlight pass. The two claims are orthogonal.
                //
                // The highlight pass is also where the result count is settled. The
                // response only knows how many statements matched; the navigation
                // UI means how many marks were painted.
                editor.update(
                    () => {
                        const keys = $applySearchResults(results, query.peek());

                        resultKeys.value = keys;
                        resultCount.value = keys.length;

                        // Clamp rather than reset. A re-search triggered by the
                        // user editing the transcript must not throw them back to
                        // the first hit --- they are typically standing on the hit
                        // they just edited. Only fall back to 0 when there was no
                        // position to keep, and to null when nothing matched.
                        const focused = focusedResult.peek();
                        focusedResult.value = keys.length === 0
                            ? null
                            : Math.min(focused ?? 0, keys.length - 1);
                    },
                    { tag: ["history-merge", SEARCH_TAG] },
                );
            }),

            // Move the caret to the focused result. Scrolling is not done here ---
            // `SearchResult` scrolls itself into view when its `focused` prop
            // flips. Scroll position follows declaratively from which mark is
            // focused; the selection is an imperative act on the document.
            focusedResult.subscribe(index => {
                if (index === null) {
                    return;
                }

                editor.update(
                    () => {
                        // Re-read inside the update rather than closing over the
                        // array. A debounced re-search may have repainted every mark
                        // between the click and this callback, which makes old keys
                        // stale --- and a stale key is an ordinary miss, not an error.
                        const key = resultKeys.peek()[index];
                        if (key === undefined) {
                            return;
                        }

                        // `?? undefined` because the type guard is written against
                        // `LexicalNode | undefined` while `$getNodeByKey` returns
                        // `| null` --- the two spellings of "absent" meet here.
                        const mark = $getNodeByKey(key) ?? undefined;
                        if (!$isSearchResultNode(mark)) {
                            return;
                        }

                        // Select the matched run rather than collapsing to a caret
                        // at its edge: what Cmd-G does in most editors, and it
                        // leaves the document one keystroke from replacing the match.
                        mark.select(0, mark.getChildrenSize());
                    },
                    // The caret move is our write, not the human's. Untagged it
                    // would reach the re-search listener below; that listener
                    // happens to ignore it (a selection change dirties no leaves),
                    // but relying on that coincidence is how the loop comes back
                    // the next time the gate is edited.
                    { tag: ["history-merge", SEARCH_TAG] },
                );
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

            // Re-run the search when the human edits the transcript, so the result
            // set and its highlights stay honest about the text actually on screen.
            //
            // This cannot be a mutation listener on SearchResultNode, for two
            // reasons worth stating because both are easy to walk back into:
            //
            //   1. Mutations do not bubble. Typing inside a highlighted run mutates
            //      the mark's TextNode child, not the mark; typing anywhere else
            //      mutates no mark at all. The one class guaranteed not to see
            //      ordinary edits is the one wrapping the matches.
            //   2. $applySearchResults destroys and recreates every mark on each
            //      result set. A listener that re-searches on mark mutations
            //      therefore feeds itself --- search, repaint, mutation, search ---
            //      forever.
            //
            // registerUpdateListener sees every commit and its `tags`, which is how
            // an editor distinguishes a human's write from its own.
            editor.registerUpdateListener(({ tags, dirtyLeaves }) => {
                // Cheapest predicate first, and the most decisive: with no query
                // there is no result set to keep honest. This must not fall through
                // to `runQuery` with an empty string --- the handler no-ops on "",
                // but only after the debounce has already scheduled a timer, which
                // would displace a pending real search.
                const pending = query.peek();
                if (!pending) {
                    return;
                }

                // Our own highlight pass, and the initial document seed, are not the
                // human changing the transcript. Reacting to SEARCH_TAG in
                // particular is the infinite loop described above.
                if (tags.has(SEARCH_TAG) || tags.has("history-merge")) {
                    return;
                }

                // Selection-only commits --- caret moves, clicks, focus changes ---
                // arrive constantly and dirty nothing. `dirtyLeaves` is the honest
                // signal for "text actually changed"; `dirtyElements` is not,
                // because it always contains `root`.
                if (dirtyLeaves.size === 0) {
                    return;
                }

                // Deliberately unscoped: any text edit anywhere re-runs the search.
                // The narrower "did this edit touch a mark" test cannot see the two
                // cases that matter most --- growing a match from adjacent text, and
                // typing a brand-new match into a statement that never held one. The
                // trailing debounce collapses a burst of keystrokes into one request.
                runQuery(pending);
            }),
        );
    },
});


const resolveSearchResultHost: ResolveHost = element =>
    element.querySelector<HTMLElement>(`:scope > .${ SEARCH_RESULT_BADGE_CLASS }`);

// The marks' React faces, portalled into each SearchResultNode's unmanaged badge
// through the shared decorator seam, plus the highlight styles they need.
//
// Deliberately does not read the `data` signal. It reacts to SearchResultNode
// mutations, which is a strictly later event: `$applySearchResults` creates the
// nodes, the reconciler builds their badge spans, the mutation listener fires,
// and only then is there anything to portal into.
function SearchResultPortals (): JSX.Element {
    const focusedResult = useExtensionSignalValue(SearchInterviewExtension, "focusedResult");
    const resultKeys = useExtensionSignalValue(SearchInterviewExtension, "resultKeys");

    // Which NodeKey is focused, resolved through the ordered key list rather than
    // by position among the portals, whose order is mutation-arrival order.
    const focusedKey = focusedResult === null ? null : resultKeys[focusedResult] ?? null;

    const portals = useNodeDecorators(SearchResultNode, resolveSearchResultHost, key => (
        <SearchResult focused={ key === focusedKey } nodeKey={ key } />
    ));

    return (
        <>
            <SearchResultStyles />
            { portals }
        </>
    );
}
