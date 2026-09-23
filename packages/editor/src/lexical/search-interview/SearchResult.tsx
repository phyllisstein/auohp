import { useEffect, useRef, type JSX } from "react";
import type { NodeKey } from "lexical";
import styled, { createGlobalStyle } from "styled-components";

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

export const SearchResultStyles = createGlobalStyle`
    /* The highlight is now painted by the <mark> itself, inline, so it flows
       with the text and breaks correctly across lines --- which is exactly what
       a background on an inline box does for free, and what no absolutely
       positioned overlay can do.

       'box-decoration-break: clone' is the piece that is easy to miss: without
       it, a highlight spanning a line break gets its padding and rounding only
       at the outer two ends, so the fragment reads as one stretched box rather
       than two. 'clone' re-applies the decoration to each line box. */
    .auohp-search-result {
        position: relative;

        margin: 0;
        padding: 0.05em 0.15em;
        border-radius: 0.2em;

        color: inherit;

        background: #FCE94F;
        box-decoration-break: clone;
    }

    /* MarkNode applies theme.markOverlap when __ids.length > 1. Every mark here
       carries exactly one uid (its statement's), so overlap only arises if a
       future caller merges ids --- worth styling now so it degrades visibly
       rather than silently. */
    .auohp-search-result.auohp-search-result--overlap {
        background: #FCAF3E;
    }
`;

// The chip's React face. It is not rendered in place by Lexical --- MarkNode is
// an ElementNode, so there is no `decorate()` hook --- it is portalled into the
// unmanaged badge span that TagChipNode.createDOM builds (see TagChipPortals).
//
// It receives only a NodeKey. Everything else is read back out of EditorState
// via `editor.read()` / `editor.update()`, which keeps the component a pure
// function of editor state rather than a second copy of it.
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
