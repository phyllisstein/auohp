import { useEffect, useState, type JSX, type ReactNode } from "react";
import { createPortal } from "react-dom";
import type { Klass, LexicalEditor, LexicalNode, NodeKey } from "lexical";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { mergeRegister } from "@lexical/utils";


// -----------------------------------------------------------------------------
// The ElementNode -> React seam, shared by every feature that paints React into
// an ElementNode's unmanaged chrome (tag chips, search results).
//
// A DecoratorNode gets React for free: the reconciler pulls JSX out of
// `decorate()` at the node's own position. Our chip-like nodes extend MarkNode,
// hence ElementNode, so no such hook exists --- and they must stay ElementNodes,
// because their children are real text (a DecoratorNode's getTextContent()
// returns "", which would silently erase those words from the statement text
// PersistenceExtension ships to the server).
//
// So the flow is inverted: a mutation listener reports each node's lifecycle,
// we resolve its unmanaged host element, and React portals into it. The host
// being `setDOMUnmanaged` is what makes this legal --- Lexical's mutation
// attribution stops there, so nothing React renders inside is evicted as
// foreign DOM.
//
// Two properties the per-feature copies this replaces did not have:
//
// 1. The portal container is a <span> we own, one per NodeKey, never the host
//    itself. `createPortal` identifies its subtree by container, so a portal
//    whose container changes remounts its children. Lexical may hand a node a
//    new element (an `updateDOM` that returns true, a full reconcile), and
//    re-portalling into the new host would reset component state. Instead the
//    slot is re-parented into whatever host is current --- `append` moves a
//    live node --- and React never learns the host changed.
// 2. "updated" mutations are handled, not skipped. An update is precisely when
//    the node may have a new element; skipping it strands the slot in a dead
//    host. An update-listener sweep re-homes any slot whose host moved without
//    a mutation record, as a second line of defense.
// -----------------------------------------------------------------------------

/** Given a node's top-level element, find the element its face lives inside. */
export type ResolveHost = (element: HTMLElement) => HTMLElement | null;

export function registerNodeDecoratorSlots (
    editor: LexicalEditor,
    nodeClass: Klass<LexicalNode>,
    resolveHost: ResolveHost,
    onChange: (slots: ReadonlyMap<NodeKey, HTMLElement>) => void,
): () => void {
    const slots = new Map<NodeKey, HTMLElement>();

    // Returns whether the key set changed; a mere re-parent does not concern
    // React.
    const ensure = (key: NodeKey): boolean => {
        const element = editor.getElementByKey(key);
        const host = element ? resolveHost(element) : null;
        if (!host) {
            return false;
        }

        let slot = slots.get(key);
        const added = !slot;
        if (!slot) {
            slot = document.createElement("span");
            slot.dataset.lexicalDecoratorSlot = key;
            slots.set(key, slot);
        }

        if (slot.parentElement !== host) {
            host.append(slot);
        }

        return added;
    };

    const drop = (key: NodeKey): boolean => {
        const slot = slots.get(key);
        if (!slot) {
            return false;
        }
        slot.remove();
        slots.delete(key);
        return true;
    };

    const publish = () => onChange(new Map(slots));

    const unregister = mergeRegister(
        editor.registerMutationListener(nodeClass, mutations => {
            let changed = false;
            for (const [key, kind] of mutations) {
                changed = (kind === "destroyed" ? drop(key) : ensure(key)) || changed;
            }
            if (changed) {
                publish();
            }
        }),

        editor.registerUpdateListener(() => {
            for (const key of slots.keys()) {
                ensure(key);
            }
        }),
    );

    return () => {
        unregister();
        for (const slot of slots.values()) {
            slot.remove();
        }
        slots.clear();
        publish();
    };
}

// Hook form for ReactExtension's `decorators` channel: returns one portal per
// live node of `nodeClass`. `resolveHost` and `nodeClass` must be stable
// (module-level) --- they are effect dependencies.
export function useNodeDecorators (
    nodeClass: Klass<LexicalNode>,
    resolveHost: ResolveHost,
    render: (key: NodeKey) => ReactNode,
): JSX.Element {
    const [editor] = useLexicalComposerContext();
    const [slots, setSlots] = useState<ReadonlyMap<NodeKey, HTMLElement>>(() => new Map());

    useEffect(
        () => registerNodeDecoratorSlots(editor, nodeClass, resolveHost, setSlots),
        [editor, nodeClass, resolveHost],
    );

    return (
        <>
            { Array.from(slots, ([key, slot]) => createPortal(render(key), slot, key)) }
        </>
    );
}
