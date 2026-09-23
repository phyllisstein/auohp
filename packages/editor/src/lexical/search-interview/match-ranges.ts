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
// fragment in SearchInterviewExtension's executor), not here.
// Escape every character the RegExp grammar treats as special, so a selection
// containing `(`, `.`, `?`, `[` and friends is matched literally rather than
// compiled as a pattern. Without this, selecting "ACT UP (1987)" throws
// SyntaxError on the unbalanced group --- a user-selectable crash.
//
// `$&` in the replacement is the whole match, so this is "prefix every special
// character with a backslash" with no capture group needed.
export const escapeRegExp = (literal: string) => literal.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

// `\b` is a zero-width assertion between a `\w` and a non-`\w`, so it only means
// what we want when the fragment's own edge characters are word characters.
// Anchoring "(1987)" with `\b` on the left would demand a word character before
// the `(` and never match. So each boundary is applied conditionally, per end.
const WORD_EDGE = /\w/;

export function findMatchRanges (text: string, fragment: string): MatchRange[] {
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
