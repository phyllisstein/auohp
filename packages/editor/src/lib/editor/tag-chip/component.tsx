import { type NodeKey } from "lexical";
import { useEffect, type JSX } from "react";
import styled from "styled-components";
import numberSignSVG from "../number.sign.square.svgo.svg?inline";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { Button } from "@react-spectrum/s2/Button";
import { INSERT_TAG_CHIP_COMMAND } from "./extension";
import { $isTagChipNode, TAG_CHIP_BADGE_CLASS } from "./node";

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

export function TagChip ({ nodeKey }: { nodeKey: NodeKey }): JSX.Element {
    return (
        <>
            <TagChipContainer data-node-key={ nodeKey } className="tag-chip__container" />
        </>
    );
}

export function TagButton (): JSX.Element {
    const [editor] = useLexicalComposerContext();

    // The payload is the mark ID. Deferred decision: a throwaway uuid for now, so
    // each chip is at least distinct. When entity resolution lands, this becomes
    // the graph uid of the Person/Organization being mentioned --- MarkNode's
    // __ids then genuinely means "this range mentions these entities", and
    // $getMarkIDs answers that question directly. No signature change needed.
    return (
        <Button
            id="insert-tag-chip"
            type="button"
            onPress={ () => editor.dispatchCommand(INSERT_TAG_CHIP_COMMAND, crypto.randomUUID()) }>
            Insert #person chip
        </Button>
    );
}
