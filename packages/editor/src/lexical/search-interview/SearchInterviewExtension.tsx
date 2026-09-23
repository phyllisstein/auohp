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
    type NodeKey,
    type TextNode,
} from "lexical";
import { namedSignals, type Signal } from "@lexical/extension";
import { $unwrapMarkNode, $wrapSelectionInMarkNode } from "@lexical/mark";
import { ReactExtension } from "@lexical/react/ReactExtension";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { useExtensionDependency } from "@lexical/react/useExtensionComponent";
import { useExtensionSignalValue, useSignalValue } from "@lexical/react/useExtensionSignalValue";
import { $dfs, $findMatchingParent, mergeRegister } from "@lexical/utils";
import { useLazyQuery } from "@apollo/client/react";
import { debounce } from "perfect-debounce";
import { useEffect, useEffectEvent, useRef, useState, type JSX } from "react";
import { createPortal } from "react-dom";
import { SEARCH_STATEMENTS_QUERY } from "~/queries";
import type { SearchStatementsQuery, SearchStatementsQueryVariables } from "~/__generated__/queries.gql";
import { PersistenceExtension } from "~/lexical/persistence/PersistenceExtension";
import { StatementExtension } from "~/lexical/statement/StatementExtension";
import { $isStatementNode, type StatementNode } from "~/lexical/statement/StatementNode";
import { TagChipPortals } from "~/lexical/tag-chip/TagChipExtension";
import { INSERT_SEARCH_RESULT_COMMAND } from "./commands";
import { findMatchRanges } from "./match-ranges";
import { SearchResult } from "./SearchResult";
import { $createSearchResultNode, $isSearchResultNode, SEARCH_RESULT_BADGE_CLASS, SearchResultNode } from "./SearchResultNode";


// Note the indexed access doing the work: `[1]["data"]` walks the tuple Apollo
// returns and pulls the field off it, so this alias tracks any future change to
// `useLazyQuery`'s result type without us restating it.
export type SearchStatementsData = ReturnType<typeof useLazyQuery<SearchStatementsQuery, SearchStatementsQueryVariables>>[1]["data"];

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
// Unlike PersistenceConfig, every field here earns its Signal: each one takes a
// second value mid-session and something reacts to it --- `data.subscribe(...)`
// in `register` repaints the highlights, the React consumers re-render off
// `.value` reads. The `.peek()` calls inside the subscriber (`query.peek()`,
// `focusedResult.peek()`) are the correct kind: reading a sibling signal's
// current value without cross-subscribing to it. Contrast PersistenceConfig's
// peeks, which unwrap a box around a value that never moves.
export interface SearchOutput {
    /** The pending search string. `null` means idle --- no search requested yet. */
    query: Signal<string | null>;
    /** Latest results, or `undefined` before the first response. */
    data: Signal<SearchStatementsData>;
    /** Whether a search is currently in flight. */
    loading: Signal<boolean>;
    /** The index of the result that has the caret, or `null` if none. */
    focusedResult: Signal<number | null>;
    /** The total number of results, or `0` if none. */
    resultCount: Signal<number>;
    /**
     * The marks painted by the last highlight pass, in document order.
     *
     * This is the authority for what "result N" means, and it exists because
     * neither of the two things that previously stood in for it is correct.
     * `resultCount` was the number of matching STATEMENTS, but a statement
     * saying "ACT UP ... ACT UP" carries two marks, so the counter and the
     * highlights disagreed about the total. And `SearchResultPortals` indexed
     * its `hosts` Map, whose insertion order is the order mutations happened to
     * arrive from the reconciler --- not document order.
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
// re-search-on-edit listener in SearchDriver would react to the repaint it just
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
        focusedResult: null,
        resultCount: 0,
        resultKeys: [] as readonly NodeKey[],
        replacement: "",
    }),

    register (editor, _config, state) {
        const { query, data, focusedResult, resultCount, resultKeys } = state.getOutput();

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
                // `query.peek()`, not `query.value`: this callback is already a
                // subscriber to `data`, and reading `.value` here would enrol it
                // as a subscriber to `query` too --- so merely typing a new
                // search would re-run the highlight pass against the OLD results.
                // `peek` reads without subscribing.
                // SEARCH_TAG rides alongside: `history-merge` says "this is not a
                // user edit" (to persistence and undo), while SEARCH_TAG says who
                // wrote it, so SearchDriver's update listener can decline to
                // re-search in response to its own highlight pass. Tags compose ---
                // the two claims are orthogonal and both are needed.
                //
                // The highlight pass is also where the result COUNT is settled,
                // rather than in `onSearchUpdate` where it used to live. The
                // response only knows how many statements matched; a statement
                // reading "ACT UP ... ACT UP" is one hit to the server and two
                // marks on the page, and the navigation UI means the second thing.
                // Counting what was painted is the only way the two agree.
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
        );
    },
});

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
    const focusedResult = useExtensionSignalValue(SearchInterviewExtension, "focusedResult");
    const resultKeys = useExtensionSignalValue(SearchInterviewExtension, "resultKeys");

    // Which NodeKey is focused, resolved through the ordered key list rather than
    // by indexing `hosts`. `hosts` is a Map filled in mutation-arrival order ---
    // reconciler order, not document order --- so its Nth entry is not reliably
    // the Nth match down the page. Comparing keys sidesteps the question of what
    // order this Map happens to be in.
    const focusedKey = focusedResult === null ? null : resultKeys[focusedResult] ?? null;

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
            { Array.from(hosts, ([key, host]) => createPortal(<SearchResult focused={ key === focusedKey } nodeKey={ key } />, host, key)) }
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
    const { query, data, loading, focusedResult, resultCount, resultKeys } = useExtensionDependency(SearchInterviewExtension).output;
    // Plain value on the output, not a signal --- read it straight off `.output`,
    // the way the SearchInterviewExtension fields above are. `useExtensionSignalValue`
    // would type-error here (`SignalValue<string>` is `never`) and blow up at
    // runtime reaching for `.subscribe` on a string.
    const { interviewUid } = useExtensionDependency(PersistenceExtension).output;
    const [editor] = useLexicalComposerContext();

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
        if (searchData && !searchData.search.statementText) {
            console.warn("SearchDriver: onSearchUpdate fired with data but no search.statementText field", searchData);
        }
        if (searchData && searchData.search.statementText) {
            console.log("SearchDriver: onSearchUpdate fired with results", searchData.search.statementText);

            // Only `data` is set here. `resultCount` and `focusedResult` used to
            // be derived from `statementText.length` at this point, which counted
            // matching STATEMENTS --- but every consumer of those signals means
            // occurrences. They are now settled by the highlight pass in
            // `register`, which is downstream of this write and can count the
            // marks it actually painted.
            data.value = searchData;
            console.log("SearchDriver: data.value updated to", data.peek());
        }
    });

    const debouncedQueryHandler = useRef(debounce(async (query: string) => {
        if (query !== "") {
            try {
                console.log("SearchDriver: debouncedQueryHandler running with query", query);
                const res = await runSearch({
                    variables: {
                        fragment: `"${ query }"`,
                        interviewUid,
                    },
                });
                console.log("SearchDriver: runSearch returned", res);
                onSearchUpdate();
            } catch (error) {
                console.warn("SearchDriver: runSearch error", error);
                loading.value = false;
            }
        }
    }, 1_500, { leading: false, trailing: true }));

    // Fires only when the user types in the search box --- a NEW query, for which
    // discarding the old position is right. The re-search-on-edit listener below
    // deliberately calls `debouncedQueryHandler` directly instead of writing
    // `query.value`, precisely so it does not land here and reset a position the
    // user is standing on. Routing that path through this signal would look like
    // a simplification and would silently reintroduce the jump-to-first-hit bug.
    useEffect(() => {
        focusedResult.value = null;
        resultCount.value = 0;
        resultKeys.value = [];

        if (!pendingQuery) {
            data.value = undefined;
            loading.value = false;
            return;
        }
        loading.value = true;
        debouncedQueryHandler.current(pendingQuery);
    }, [pendingQuery]);

    // Re-run the search when the human edits the transcript, so the result set
    // and its highlights stay honest about the text actually on screen.
    //
    // This CANNOT be a mutation listener on SearchResultNode, for two reasons
    // worth stating because both are easy to walk back into:
    //
    //   1. Mutations do not bubble. Typing inside a highlighted run mutates the
    //      mark's TextNode child, not the mark; typing anywhere else mutates no
    //      mark at all. The one class guaranteed NOT to see ordinary edits is
    //      the one wrapping the matches.
    //   2. $applySearchResults destroys and recreates every mark on each result
    //      set. A listener that re-searches on mark mutations therefore feeds
    //      itself --- search, repaint, mutation, search --- forever.
    //
    // registerUpdateListener sees every commit and, crucially, its `tags`, which
    // is how an editor distinguishes a human's write from its own. SEARCH_TAG is
    // stamped on the highlight pass so this listener can decline to react to it.
    useEffect(
        () =>
            editor.registerUpdateListener(({ tags, dirtyLeaves }) => {
                // Cheapest predicate first, and it is also the most decisive: with
                // no query there is no result set to keep honest, so an edit made
                // with the search bar closed costs nothing at all. Note this must
                // not fall through to `debouncedQueryHandler` with an empty string
                // --- the handler no-ops on "", but only AFTER the debounce has
                // already scheduled a timer, which would displace a pending real
                // search. `peek`, not `.value`: a listener is not a reactive
                // context, and subscribing here would be meaningless anyway.
                const pending = query.peek();
                if (!pending) {
                    return;
                }

                // Our own highlight pass, and the initial document seed, are not
                // the human changing the transcript. Reacting to SEARCH_TAG in
                // particular is the infinite loop described above.
                if (tags.has(SEARCH_TAG) || tags.has("history-merge")) {
                    return;
                }

                // Selection-only commits --- caret moves, clicks, focus changes ---
                // arrive here constantly and dirty nothing. `dirtyLeaves` is the
                // honest signal for "text actually changed"; `dirtyElements` is
                // not, because it always contains `root` (every commit reconciles
                // from the top), so testing it would make this gate vacuous.
                if (dirtyLeaves.size === 0) {
                    return;
                }

                // Deliberately unscoped: any text edit anywhere re-runs the search.
                // The narrower "did this edit touch a mark" test cannot see the two
                // cases that matter most --- growing a match from adjacent text
                // ("I [ACT UP] in" -> "I [ACT UP]ped in", where the dirty leaf is
                // the neighbour, not the mark), and typing a brand-new match into a
                // statement that has never held one. The trailing debounce already
                // collapses a burst of keystrokes into a single round-trip, so the
                // cost of being permissive is one query per typing pause.
                debouncedQueryHandler.current(pending);
            }),
        [editor, query],
    );

    // Move the caret to the focused result.
    //
    // Subscribing to the signal directly, rather than reading it through
    // `useExtensionSignalValue`, is deliberate: this component renders nothing,
    // and hooking the value into render would make every Next/Previous click
    // re-render it for no visual purpose. Moving the caret is a side effect on
    // the editor, so it belongs on the subscription, not on a render pass.
    //
    // Scrolling is NOT done here --- `SearchResult` already scrolls itself into
    // view when its `focused` prop flips (see SearchResult.tsx). That split is worth
    // keeping: scroll position follows declaratively from which mark is focused,
    // while the selection is an imperative act on the document.
    useEffect(
        () =>
            focusedResult.subscribe(index => {
                if (index === null) {
                    return;
                }

                editor.update(
                    () => {
                        // Re-read inside the update rather than closing over the
                        // array. A debounced re-search may have repainted every
                        // mark between the click and this callback, which makes
                        // the old keys stale --- and a stale key is not an error
                        // here, just a miss, so `$getNodeByKey` returning null is
                        // an ordinary outcome to bail on rather than to guard
                        // against upstream.
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

                        // Select the matched run rather than collapsing to a
                        // caret at its edge. This is what Cmd-G does in most
                        // editors, it makes the current hit visible as a
                        // selection even before any highlight styling, and it
                        // leaves the document one keystroke from replacing the
                        // match --- which is the seam find-and-replace will use.
                        mark.select(0, mark.getChildrenSize());
                    },
                    // The caret move is our write, not the human's. Untagged it
                    // would reach the re-search listener above; that listener
                    // happens to ignore it (a selection change dirties no
                    // leaves), but relying on that coincidence is how the loop
                    // comes back the next time the gate is edited.
                    { tag: ["history-merge", SEARCH_TAG] },
                );
            }),
        [editor, focusedResult, resultKeys],
    );

    return null;
}
