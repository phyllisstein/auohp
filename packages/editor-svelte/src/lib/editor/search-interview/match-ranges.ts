// A character range within a statement's flattened text, half-open: [start, end).
export interface MatchRange {
    start: number;
    end: number;
}

// Escape every character the RegExp grammar treats as special, so a selection
// containing `(`, `.`, `?`, `[` and friends is matched literally. `$&` in the
// replacement is the whole match, so this is "prefix every special character
// with a backslash" with no capture group needed.
export const escapeRegExp = (literal: string) => literal.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

// `\b` is a zero-width assertion between a `\w` and a non-`\w`, so it only
// means what we want when the fragment's own edge characters are word
// characters -- anchoring "(1987)" with `\b` on the left would demand a word
// character before the `(` and never match. Each boundary is applied
// conditionally, per end.
export const WORD_EDGE = /\w/;

// The server cannot tell us where a match falls within a statement's text --
// db.index.fulltext.queryNodes scores whole nodes and returns them, not the
// token -> offset mapping Lucene built internally. So ranges are recomputed
// here, from the text we already have.
export function findMatchRanges (text: string, fragment: string): MatchRange[] {
    // A zero-length fragment makes a global RegExp match the empty string at
    // every position -- N zero-width ranges, an infinite loop in a hand-rolled
    // scan, and nothing to highlight regardless.
    const needle = fragment.trim();
    if (needle.length === 0) {
        return [];
    }

    const pattern = new RegExp(
        (WORD_EDGE.test(needle.at(0)!) ? "\\b" : "") +
        escapeRegExp(needle) +
        (WORD_EDGE.test(needle.at(-1)!) ? "\\b" : ""),
        // `i` because Lucene's `standard` analyzer lowercases both sides -- a
        // statement returned for "act up" may read "ACT UP". A RegExp rather
        // than `text.toLowerCase().indexOf(...)` is load-bearing:
        // `toLowerCase` is not length-preserving in general (U+0130 lowercases
        // to two code units), so offsets in a lowercased copy can drift out of
        // alignment with `text`. `matchAll` reports `index` in the ORIGINAL
        // string's coordinates.
        "gi",
    );

    const ranges: MatchRange[] = [];
    let lastEnd = 0;

    for (const match of text.matchAll(pattern)) {
        const start = match.index;
        const end = start + match[0].length;

        // Drop anything overlapping the previous accepted range. A fixed-
        // length literal cannot self-overlap under matchAll, but the
        // invariant is asserted rather than assumed: $markMatchesInStatement
        // derives split points from these boundaries, and overlapping ranges
        // would produce cuts that interleave into nonsense pieces.
        if (start < lastEnd) {
            continue;
        }

        ranges.push({ start, end });
        lastEnd = end;
    }

    return ranges;
}
