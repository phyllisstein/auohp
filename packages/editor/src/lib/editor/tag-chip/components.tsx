import { type JSX, useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { type NodeKey } from "lexical";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { Button } from "@react-spectrum/s2/Button";
import styled from "styled-components";
import numberSignSVG from "./number.sign.square.svgo.svg?inline";
import { INSERT_TAG_CHIP_COMMAND } from "./commands";
import { TAG_CHIP_BADGE_CLASS, TagChipNode } from "./node";

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

// A trivial toolbar affordance that dispatches the typed insert command ---
// demonstrating an out-of-editor React control mutating EditorState, which then
// renders back through a React DecoratorNode. `useLexicalComposerContext` is not
// deprecated: it remains the sanctioned way for a component that genuinely
// renders something to reach the editor (useExtensionComponent is built on it).
// What was deprecated is using it as a back door for behaviour-only components.
export function TagButton (): JSX.Element {
    const [editor] = useLexicalComposerContext();

    // The payload is the mark ID. Deferred decision: a throwaway uid for now, so
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

// -----------------------------------------------------------------------------
// TagChipPortals --- the ElementNode -> React bridge.
//
// A DecoratorNode gets React for free: the reconciler pulls JSX out of
// `decorate()` at the node's own position. TagChipNode extends MarkNode, hence
// ElementNode, so no such hook exists --- and it must stay an ElementNode,
// because its children are the tagged text (a DecoratorNode's getTextContent()
// returns "", which would silently erase those words from the statement text
// PersistenceExtension ships to the server).
//
// So we invert the flow and push instead: a mutation listener reports every
// chip's lifecycle, we resolve each one's unmanaged badge span, and React
// portals into it. The badge being `setDOMUnmanaged` is what makes this legal
// --- Lexical's mutation-attribution up-walk terminates there, so nothing React
// renders inside gets evicted as foreign DOM.
//
// Rendered via ReactExtension's `decorators` channel, which exists precisely for
// "JSX inside the editor context that is not location-dependent".
// -----------------------------------------------------------------------------
export function TagChipPortals (): JSX.Element {
    const [editor] = useLexicalComposerContext();

    // NodeKey -> the badge span to portal into. Held in React state (not a ref)
    // because adding or dropping an entry must trigger a re-render.
    const [hosts, setHosts] = useState<ReadonlyMap<NodeKey, HTMLElement>>(new Map());

    useEffect(
        () =>
            editor.registerMutationListener(TagChipNode, mutations => {
                // Resolve a chip's portal target from its NodeKey. Returns null
                // when the node has no DOM yet (or no badge, e.g. a node
                // replacement swapped createDOM out from under us).
                const resolveHost = (key: NodeKey): HTMLElement | null =>
                    editor.getElementByKey(key)?.querySelector<HTMLElement>(
                        `:scope > .${ TAG_CHIP_BADGE_CLASS }`,
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
            { Array.from(hosts, ([key, host]) => createPortal(<TagChip nodeKey={ key } />, host, key)) }
        </>
    );
}
