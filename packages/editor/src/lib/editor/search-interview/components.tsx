import { type JSX, useCallback, useEffect, useEffectEvent, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { $getNodeByKey, type NodeKey } from "lexical";
import { useLazyQuery } from "@apollo/client/react";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { useExtensionDependency } from "@lexical/react/useExtensionComponent";
import { useExtensionSignalValue, useSignalValue } from "@lexical/react/useExtensionSignalValue";
import { ActionButton, Text } from "@react-spectrum/s2/ActionButton";
import { ActionButtonGroup } from "@react-spectrum/s2/ActionButtonGroup";
import ChevronDownIcon from "@react-spectrum/s2/icons/ChevronDown";
import ChevronUpIcon from "@react-spectrum/s2/icons/ChevronUp";
import SearchIcon from "@react-spectrum/s2/icons/Search";
import { ProgressCircle } from "@react-spectrum/s2/ProgressCircle";
import { style } from "@react-spectrum/s2/style" with { type: "macro" };
import { TextField } from "@react-spectrum/s2/TextField";
import styled from "styled-components";
import { debounce } from "perfect-debounce";
import { SEARCH_STATEMENTS_QUERY } from "~/queries";
import { PersistenceExtension } from "../persistence";
import { $replaceMark, SEARCH_TAG, SearchInterviewExtension } from "./extension";
import { $isSearchResultNode, SEARCH_RESULT_BADGE_CLASS, SearchResultNode } from "./node";

// The badge host's face. It stays a portal target (so a per-match affordance ---
// hit counter, jump-to-next, hover popover --- has somewhere to live) but it no
// longer paints the highlight itself.
//
// The previous geometry gave this away: `position: absolute; left: -1em;
// width: calc(100% + 1.6em)` drew a band spanning the whole statement, which was
// coherent only while the mark WRAPPED the whole statement. A mark that wraps a
// few words inside a sentence cannot be painted by an absolutely-positioned box
// --- an inline run that wraps across a line break is not one rectangle, and
// `100%` of an inline box is not the width you want anyway.
const SearchResultContainer = styled.span`
    user-select: none;

    /* Out of flow without being positioned: the badge must not consume layout
       space between the words it sits inside. It is a hook for portalled
       chrome, so it has no size of its own until something is portalled in. */
    display: inline;

    font-size: 100%;
    font-weight: 600;
    color: #0B0B0B;
`;

// The search result's React face. It is not rendered in place by Lexical ---
// MarkNode is an ElementNode, so there is no `decorate()` hook --- it is
// portalled into the unmanaged badge span that SearchResultNode.createDOM builds
// (see SearchResultPortals).
//
// It receives only a NodeKey and whether it is the focused result, and uses the
// latter solely to scroll itself into view. The highlight is painted by the mark
// itself (see SearchResultStyles), not by this component.
export function SearchResult ({ nodeKey, focused }: { nodeKey: NodeKey; focused: boolean }): JSX.Element {
    const container = useRef<HTMLSpanElement>(null);

    useEffect(() => {
        if (focused && container.current) {
            container.current.scrollIntoView({ behavior: "smooth", block: "center" });
        }
    }, [focused]);

    return (
        <SearchResultContainer ref={ container } data-node-key={ nodeKey } className="auohp-search-result__container" />
    );
}

const SearchContainer = styled.div`
    position: fixed;
    z-index: 1000;
    right: 0;

    display: flex;
    flex-direction: column;
    gap: 1rem;
    align-items: flex-end;
    justify-content: center;

    width: 100%;
    height: max-content;
    min-height: max-content;
    padding: 1rem;
`;

const SearchFieldContainer = styled.div`
    width: 50%;
`;

const ButtonGroupContainer = styled.div`
    display: flex;
    align-items: center;
    justify-content: flex-end;
    width: max-content;
`;

export function SearchBar (): JSX.Element {
    const { query, focusedResult, resultKeys, replacement } = useExtensionDependency(SearchInterviewExtension).output;
    const queryValue = useExtensionSignalValue(SearchInterviewExtension, "query");
    const loading = useExtensionSignalValue(SearchInterviewExtension, "loading");
    const resultCount = useExtensionSignalValue(SearchInterviewExtension, "resultCount");
    const focusedResultValue = useExtensionSignalValue(SearchInterviewExtension, "focusedResult");
    const replacementValue = useExtensionSignalValue(SearchInterviewExtension, "replacement");
    const [editor] = useLexicalComposerContext();

    const spinner = (
        <ProgressCircle
            aria-label="Loading…"
            value={ 80 }
            isIndeterminate
            size="S"
            staticColor="white" />
    );

    // Both directions wrap. `%` after adding `resultCount` keeps the operand
    // non-negative --- JavaScript's `%` is a remainder, not a modulus, so a bare
    // `(current - 1) % n` yields -1 at the top of the list rather than n-1.
    //
    // The guard matters because `resultCount` now counts painted marks, which is
    // 0 whenever the query matches nothing; without it both handlers would
    // compute NaN and poison the signal. The buttons are disabled in that state,
    // but a keyboard shortcut bound to these later would not be.
    const focusNext = useCallback(() => {
        if (resultCount === 0) {
            return;
        }
        focusedResult.value = ((focusedResult.peek() ?? -1) + 1) % resultCount;
    }, [focusedResult, resultCount]);

    const focusPrevious = useCallback(() => {
        if (resultCount === 0) {
            return;
        }
        focusedResult.value = ((focusedResult.peek() ?? 0) - 1 + resultCount) % resultCount;
    }, [focusedResult, resultCount]);

    // Replace the focused match.
    //
    // Note what is NOT here: no tag. Every other `editor.update` in this
    // extension carries `history-merge` to tell PersistenceExtension "no text
    // changed, do not save" --- true of the highlight pass, which only wraps
    // runs in marks. Replace is the opposite and must stay untagged so that all
    // three downstream listeners fire: persistence saves the statement, history
    // records an undo step, and the re-search listener repaints the results.
    //
    // That last one is why nothing here re-runs the search by hand. Unwrapping
    // the mark dirties a leaf, the update listener sees an untagged commit with
    // dirty leaves, and the debounced search follows on its own.
    const replaceFocused = useCallback(() => {
        const index = focusedResult.peek();
        if (index === null) {
            return;
        }

        editor.update(() => {
            const key = resultKeys.peek()[index];
            if (key === undefined) {
                return;
            }

            const mark = $getNodeByKey(key) ?? undefined;
            if (!$isSearchResultNode(mark)) {
                return;
            }

            $replaceMark(mark, replacement.peek());
        });

        // Hold the index rather than advancing it. The replaced match leaves the
        // result set, so the NEXT match slides into this position --- keeping the
        // index puts the user on it, which is what repeated Replace clicks want.
        // Clamping against the new count happens in the highlight pass.
    }, [editor, focusedResult, resultKeys, replacement]);

    // Replace every match, in one update so it is one undo step and one save per
    // statement rather than one per occurrence.
    //
    // Iterating the key list is safe even though each `$replaceMark` unwraps a
    // mark: `resultKeys` is a plain array captured before the walk, and the keys
    // it names are independent nodes. Resolving each key inside the loop (rather
    // than resolving all the nodes up front) means a mark already removed as a
    // side effect of an earlier replacement simply misses.
    const replaceAll = useCallback(() => {
        const keys = resultKeys.peek();
        if (keys.length === 0) {
            return;
        }

        editor.update(() => {
            const value = replacement.peek();

            for (const key of keys) {
                const mark = $getNodeByKey(key) ?? undefined;
                if (!$isSearchResultNode(mark)) {
                    continue;
                }
                $replaceMark(mark, value);
            }
        });
    }, [editor, resultKeys, replacement]);
    return (
        <SearchContainer>
            <SearchFieldContainer>
                <TextField
                    aria-label="Search transcript"
                    type="search"
                    enterKeyHint="search"
                    inputMode="search"
                    prefix={ loading ? spinner : <SearchIcon /> }
                    size="M"
                    value={ queryValue ?? "" }
                    onChange={ value =>
                        // Writing the signal is the request; SearchDriver is what makes it a network call.
                        query.value = value.length > 0 ? value : null } />
            </SearchFieldContainer>
            <ButtonGroupContainer>
                { resultCount > 0 && (
                    <span style={{ padding: "0 1rem" }} className={ style({ color: "detail", fontSize: "detail" }) }>
                        { (focusedResultValue ?? 0) + 1 } of { resultCount }
                    </span>
                ) }
                <ActionButtonGroup isDisabled={ queryValue === null || loading || !resultCount }>
                    <ActionButton onPress={ focusPrevious }>
                        <ChevronUpIcon />
                        <Text>Previous</Text>
                    </ActionButton>
                    <ActionButton onPress={ focusNext }>
                        <ChevronDownIcon />
                        <Text>Next</Text>
                    </ActionButton>
                </ActionButtonGroup>
            </ButtonGroupContainer>
            <SearchFieldContainer>
                <TextField
                    aria-label="Replace with"
                    type="text"
                    inputMode="text"
                    size="M"
                    value={ replacementValue }
                    onChange={ value => replacement.value = value } />
            </SearchFieldContainer>
            <ButtonGroupContainer>
                { /* Deliberately not disabled on an empty replacement: clearing
                     a match is a legitimate edit, and "replace with nothing" is
                     how you delete a repeated filler word across a transcript. */ }
                <ActionButtonGroup isDisabled={ queryValue === null || loading || !resultCount }>
                    <ActionButton onPress={ replaceFocused }>
                        <Text>Replace</Text>
                    </ActionButton>
                    <ActionButton onPress={ replaceAll }>
                        <Text>Replace All</Text>
                    </ActionButton>
                </ActionButtonGroup>
            </ButtonGroupContainer>
        </SearchContainer>
    );
}

// Deliberately does not read the `data` signal. It reacts to SearchResultNode
// mutations, which is a strictly later event: `$applySearchResults` creates the
// nodes, the reconciler builds their badge spans, the mutation listener fires,
// and only then is there anything to portal into. Reading `data` here as well
// would close a loop --- create nodes -> mutation -> setHosts -> re-render ->
// create nodes --- and conflate "own the marks" with "paint inside the marks".
export function SearchResultPortals (): JSX.Element {
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
export function SearchDriver (): JSX.Element | null {
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
    // view when its `focused` prop flips (see nodes.tsx). That split is worth
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
