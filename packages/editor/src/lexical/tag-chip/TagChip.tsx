import type { JSX } from "react";
import type { NodeKey } from "lexical";
import styled, { createGlobalStyle } from "styled-components";
import numberSignSVG from "../number.sign.square.svgo.svg?inline";

const TagChipContainer = styled.span`
    user-select: none;

    position: absolute;
    z-index: -1;
    top: 0;
    left: -1em;

    display: block;

    width: calc(100% + 1.6em);
    height: 100%;

    font-size: 100%;
    font-weight: 600;
    color: #0B0B0B;

    background: #7DD3FC;

    &::before {
        content: ${ () => `url("${ numberSignSVG }") ` };

        position: absolute;
        left: 0;

        display: block;

        width: 0.8em;
        height: 0.8em;

        color: #000;

        fill: #000;
        stroke: #000;
    }
`;

export const TagMarkStyles = createGlobalStyle`
    .auohp-tag-chip {
        position: relative;
        display: inline-block;
        margin: 0 1.5rem;
        background: none;
    }
`;

// The chip's React face. It is not rendered in place by Lexical --- MarkNode is
// an ElementNode, so there is no `decorate()` hook --- it is portalled into the
// unmanaged badge span that TagChipNode.createDOM builds (see TagChipPortals).
//
// It receives only a NodeKey. Everything else is read back out of EditorState
// via `editor.read()` / `editor.update()`, which keeps the component a pure
// function of editor state rather than a second copy of it.
export function TagChip ({ nodeKey }: { nodeKey: NodeKey }): JSX.Element {
    return (
        <>
            <TagChipContainer data-node-key={ nodeKey } className="tag-chip__container" />
        </>
    );
}
