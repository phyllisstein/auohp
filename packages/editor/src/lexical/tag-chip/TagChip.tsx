import { useCallback, useSyncExternalStore, type JSX } from "react";
import { $getNodeByKey, type NodeKey } from "lexical";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import styled, { createGlobalStyle } from "styled-components";
import { $isTagChipNode } from "./TagChipNode";

// A small dot at the head of the tagged run, inline and in flow. The previous
// face was an absolutely positioned band sized to `100%` of the mark, which only
// worked while the mark was an inline-block with generous side margins to make
// room for it --- and an inline-block cannot wrap across lines, so a long tag
// pushed its whole run onto the next line.
const Badge = styled.span`
    user-select: none;

    display: inline-flex;
    align-items: center;
    vertical-align: baseline;

    margin-inline-end: 0.2em;
`;

const Dot = styled.span`
    display: inline-block;

    width: 0.55em;
    height: 0.55em;
    border-radius: 50%;

    background: var(--auohp-tag-chip-color, #B36);
`;

// The <mark> itself stays inline so a tag wraps with the text like any other
// run. `color`/`background` neutralise the user agent's yellow <mark>, which would
// otherwise read as a search hit; the underline carries the tag's extent, in the
// dot's color, and `box-decoration-break: clone` repeats it on every line box.
export const TagChipStyles = createGlobalStyle`
    .auohp-tag-chip {
        color: inherit;

        background: none;
        text-decoration: underline 0.1em var(--auohp-tag-chip-color, #B36);
        text-underline-offset: 0.2em;
        box-decoration-break: clone;
    }
`;

// The chip's React face. It is not rendered in place by Lexical --- MarkNode is
// an ElementNode, so there is no `decorate()` hook --- it is portalled into the
// unmanaged badge span that TagChipNode.createDOM builds (see TagChipPortals).
//
// It receives only a NodeKey and reads its ids back out of EditorState, so the
// component is a function of editor state rather than a second copy of it.
// `useSyncExternalStore` is the whole subscription: the update listener is the
// store's `subscribe`, and the snapshot is the ids joined into a string --- a
// primitive, so an update that leaves them unchanged compares equal and skips
// the render.
export function TagChip ({ nodeKey }: { nodeKey: NodeKey }): JSX.Element {
    const [editor] = useLexicalComposerContext();

    const subscribe = useCallback((onChange: () => void) => editor.registerUpdateListener(onChange), [editor]);
    const ids = useSyncExternalStore(subscribe, () =>
        editor.read(() => {
            const node = $getNodeByKey(nodeKey) ?? undefined;
            return $isTagChipNode(node) ? node.getIDs().join(", ") : "";
        }),
    );

    return (
        <Badge data-node-key={ nodeKey } title={ ids }>
            <Dot />
        </Badge>
    );
}
