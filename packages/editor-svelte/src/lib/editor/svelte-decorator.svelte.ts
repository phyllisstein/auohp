import { mount, unmount, type Component } from "svelte";
import type { Klass, LexicalEditor, LexicalNode, NodeKey } from "lexical";

// -----------------------------------------------------------------------------
// registerSvelteDecorator --- the Svelte analogue of @lexical/react's
// ReactExtension `decorators` channel.
//
// React's `createPortal(vnode, host, key)` keeps a component subtree alive
// across a host change because the subtree's identity is (parent fiber, key),
// not the host element. Svelte 5's `mount(Component, { target })` has no such
// indirection: the instance is bound to the target it was given, there is no
// `setTarget`, and `unmount()` destroys state.
//
// So don't move the component -- move the DOM. Mount once into a detached
// <span> we own forever, then re-parent that span into whatever host Lexical
// currently offers. appendChild moves a live node, so every effect, binding
// and listener inside it survives, because nothing was ever destroyed.
// -----------------------------------------------------------------------------

export interface DecoratorSpec<Props extends Record<string, unknown>> {
    /** The Svelte component to mount per node. */
    component: Component<Props>;
    /** Build the props for a given NodeKey. Called once, at mount. */
    props: (key: NodeKey) => Props;
    /**
     * Given the node's top-level element, find the element the decorator should
     * live inside. Return null when the node has no usable host yet.
     */
    resolveHost: (element: HTMLElement) => HTMLElement | null;
}

interface Entry {
    /** The span we own and mount into. Never re-created for a given NodeKey. */
    slot: HTMLElement;
    /** Whatever `unmount` needs. */
    instance: Record<string, unknown>;
    /** Last host we parked the slot in, so we can skip no-op re-parents. */
    host: HTMLElement | null;
}

/**
 * Register a per-node Svelte decorator, keyed by NodeKey, driven by a Lexical
 * mutation listener. Returns a teardown that unmounts everything.
 */
export function registerSvelteDecorator<Props extends Record<string, unknown>>(
    editor: LexicalEditor,
    nodeClass: Klass<LexicalNode>,
    spec: DecoratorSpec<Props>,
): () => void {
    const entries = new Map<NodeKey, Entry>();

    const ensure = (key: NodeKey): Entry | null => {
        const element = editor.getElementByKey(key);
        if (!element) {
            return null;
        }

        const host = spec.resolveHost(element);
        if (!host) {
            return null;
        }

        let entry = entries.get(key);

        if (!entry) {
            // First sighting of this NodeKey. Create the slot detached, mount
            // into it, THEN attach -- Svelte does not require the target to be
            // in the document.
            const slot = document.createElement("span");
            slot.dataset.svelteSlot = key;
            const instance = mount(spec.component, {
                target: slot,
                props: spec.props(key),
            }) as Record<string, unknown>;
            entry = { slot, instance, host: null };
            entries.set(key, entry);
        }

        // The load-bearing line. If Lexical rebuilt the host DOM, the slot is
        // orphaned (or parked in the dead host); appendChild moves it into the
        // live one, taking the component's state, effects and listeners along
        // -- the node itself was never destroyed, only re-parented.
        if (entry.host !== host || entry.slot.parentElement !== host) {
            host.append(entry.slot);
            entry.host = host;
        }

        return entry;
    };

    const drop = (key: NodeKey) => {
        const entry = entries.get(key);
        if (!entry) {
            return;
        }
        unmount(entry.instance);
        entry.slot.remove();
        entries.delete(key);
    };

    const unregisterMutations = editor.registerMutationListener(nodeClass, mutations => {
        for (const [key, kind] of mutations) {
            if (kind === "destroyed") {
                drop(key);
            } else {
                // "created" AND "updated" both run through ensure(). An update
                // is precisely when Lexical may have handed the node a new
                // element, so skipping it (as the React portal version does)
                // would miss exactly the case that matters.
                ensure(key);
            }
        }
    });

    // Insurance, not a demonstrated fix: setRootElement(null) commits via
    // resetEditor, which nulls the mutation observer and clears textContent
    // directly before $commitPendingUpdates runs -- $reconcileRoot never
    // executes, so the mutation listener sees nothing for that commit.
    // Measured in lexical 0.49.0: the follow-up setRootElement(reattach) then
    // fires FULL_RECONCILE, which re-announces every live node as "created"
    // regardless, so the mutation listener alone already recovers once
    // reattach happens -- this sweep has not been shown to change that
    // outcome. It stays as a second line of defense against orphaned slots in
    // case some future Lexical build (or an in-between read while detached)
    // exercises a path the mutation listener misses: one getElementByKey per
    // live decorator, re-parenting if the host moved.
    const unregisterUpdates = editor.registerUpdateListener(() => {
        for (const key of [...entries.keys()]) {
            const element = editor.getElementByKey(key);
            if (!element) {
                // Node still exists in state but has no DOM (collapsed,
                // off-screen). Leave the entry alone; the next update re-homes
                // it.
                continue;
            }
            ensure(key);
        }
    });

    return () => {
        unregisterMutations();
        unregisterUpdates();
        for (const key of [...entries.keys()]) {
            drop(key);
        }
    };
}
