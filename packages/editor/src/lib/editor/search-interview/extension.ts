import {
    $createTextNode,
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
import { $dfs, $findMatchingParent, mergeRegister } from "@lexical/utils";
import { $isStatementNode, StatementExtension, type StatementNode } from "../statement";
import { type SearchStatementsData } from "../shared";
import { INSERT_SEARCH_RESULT_COMMAND } from "./commands";
import { SearchDriver, SearchResultPortals } from "./components";
import { $createSearchResultNode, $isSearchResultNode, SearchResultNode } from "./node";

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

// A character range within a statement's flattened text, half-open: `[start, end)`.
export interface MatchRange {
    start: number;
    end: number;
}


// Where the highlight ranges come from.
//
// The server cannot tell us. `db.index.fulltext.queryNodes` scores whole
// Statement nodes against the Lucene index and returns the node --- the
// token -> character-offset mapping Lucene built while analysing the text is
// internal to the index and never surfaces through Cypher. `SearchHit` carries
// `statement { uid, text }`, and that is the whole of it.
//
// So the ranges are recomputed here, from the text we already have. That is
// only defensible because the index is created with no analyzer argument
// (`CREATE FULLTEXT INDEX statementText ... ON EACH [s.text]` in api/src/main.rs),
// which means Neo4j's default `standard` analyzer: it lowercases and splits on
// non-word boundaries, but does NOT stem and does NOT strip stopwords. Had the
// index been built with the `english` analyzer, "organizing" would index as the
// stem "organ" and match a statement reading "organized" --- and a literal scan
// for "organizing" would find nothing to highlight in a statement that
// legitimately matched.
//
// One divergence survives and is accepted by design: the fragment is sent to
// Lucene unquoted, so a multi-word selection parses as OR'd terms and a
// statement matching only one of them is still a hit. Such a statement is
// returned with no literal occurrence of the full fragment, and therefore gets
// no highlight. Closing that gap belongs at the query (phrase-quoting the
// fragment in SearchDriver), not here.
// Escape every character the RegExp grammar treats as special, so a selection
// containing `(`, `.`, `?`, `[` and friends is matched literally rather than
// compiled as a pattern. Without this, selecting "ACT UP (1987)" throws
// SyntaxError on the unbalanced group --- a user-selectable crash.
//
// `$&` in the replacement is the whole match, so this is "prefix every special
// character with a backslash" with no capture group needed.
const escapeRegExp = (literal: string) => literal.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

// `\b` is a zero-width assertion between a `\w` and a non-`\w`, so it only means
// what we want when the fragment's own edge characters are word characters.
// Anchoring "(1987)" with `\b` on the left would demand a word character before
// the `(` and never match. So each boundary is applied conditionally, per end.
const WORD_EDGE = /\w/;

function findMatchRanges (text: string, fragment: string): MatchRange[] {
    // A zero-length fragment makes a global RegExp match the empty string at
    // every position, yielding N zero-width ranges and an infinite loop in any
    // hand-rolled scan. There is also nothing to highlight.
    const needle = fragment.trim();
    if (needle.length === 0) {
        return [];
    }

    const pattern = new RegExp(
        (WORD_EDGE.test(needle.at(0)!) ? "\\b" : "") +
        escapeRegExp(needle) +
        (WORD_EDGE.test(needle.at(-1)!) ? "\\b" : ""),
        // `g` to find every occurrence, `i` because Lucene's `standard` analyzer
        // lowercases both sides --- a statement returned for "act up" may well
        // read "ACT UP", and matching case-sensitively would render a hit with no
        // highlight at all.
        //
        // Doing this with a RegExp rather than `text.toLowerCase().indexOf(...)`
        // is the load-bearing choice: `toLowerCase` is not length-preserving in
        // general (U+0130 LATIN CAPITAL LETTER I WITH DOT ABOVE lowercases to two
        // code units), so offsets found in the lowercased copy can drift out of
        // alignment with `text`. `matchAll` reports `index` in the ORIGINAL
        // string's coordinates, which is exactly what $markMatchesInStatement
        // needs.
        "gi",
    );

    const ranges: MatchRange[] = [];
    let lastEnd = 0;

    for (const match of text.matchAll(pattern)) {
        const start = match.index;
        const end = start + match[0].length;

        // Drop anything that overlaps the previous accepted range. `matchAll`
        // already advances past each match so a fixed-length literal cannot
        // self-overlap, but the invariant is asserted here rather than assumed:
        // $markMatchesInStatement derives splitText cut points from these
        // boundaries, and overlapping ranges would produce cuts that interleave
        // into nonsense pieces.
        if (start < lastEnd) {
            continue;
        }

        ranges.push({ start, end });
        lastEnd = end;
    }

    return ranges;
}


// Stamped on the `editor.update()` that paints search highlights, so listeners
// can tell the search's own writes apart from a human's typing. Without it the
// re-search-on-edit listener in SearchDriver would react to the repaint it just
// caused and spin forever --- the marks it watches are destroyed and recreated
// on every result set.
export const SEARCH_TAG = "auohp-search-highlight";


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
        configExtension(ReactExtension, { decorators: [SearchResultPortals, SearchDriver] }),
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
