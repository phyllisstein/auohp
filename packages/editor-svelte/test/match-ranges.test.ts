import { describe, expect, it } from "vitest";

import { findMatchRanges } from "../src/lib/editor/search-interview/match-ranges";

describe("findMatchRanges", () => {
    it("finds a plain substring match", () => {
        expect(findMatchRanges("the quick brown fox", "quick")).toEqual([
            { start: 4, end: 9 },
        ]);
    });

    it("matches case-insensitively, Lucene standard-analyzer style", () => {
        expect(findMatchRanges("ACT UP raised its voice", "act up")).toEqual([
            { start: 0, end: 6 },
        ]);
    });

    it("does not require a word character across a punctuation edge", () => {
        // "1987" is flanked by "(" and ")", neither a \w character -- a
        // literal \b anchor on both sides would never match here.
        expect(findMatchRanges("the demonstration (1987) drew thousands", "1987")).toEqual([
            { start: 19, end: 23 },
        ]);
    });

    it("does not throw on a fragment containing regex metacharacters", () => {
        expect(() => findMatchRanges("say (hello) to everyone", "(hello)")).not.toThrow();
        expect(findMatchRanges("say (hello) to everyone", "(hello)")).toEqual([
            { start: 4, end: 11 },
        ]);
    });

    it("returns an empty array for an empty or whitespace-only fragment", () => {
        expect(findMatchRanges("some text", "")).toEqual([]);
        expect(findMatchRanges("some text", "   ")).toEqual([]);
    });

    it("never reports two overlapping ranges from a single call", () => {
        // The overlap guard (`start < lastEnd`) protects an invariant that
        // this codebase cannot presently construct a failing case for:
        // `matchAll` on a fixed-width literal pattern always resumes
        // scanning at the END of the previous match, so two candidates from
        // one findMatchRanges call cannot overlap for any fragment. This
        // test asserts the invariant directly against several back-to-back
        // matches, so it still catches a regression if the pattern
        // construction ever grows a variable-width or lookaround piece that
        // makes overlap possible.
        const ranges = findMatchRanges("(.)(.)(.)(.)", "(.)");
        for (let i = 1; i < ranges.length; i++) {
            expect(ranges[i].start).toBeGreaterThanOrEqual(ranges[i - 1].end);
        }
        expect(ranges.length).toBeGreaterThan(0);
    });
});
