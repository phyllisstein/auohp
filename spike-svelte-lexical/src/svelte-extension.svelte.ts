// -----------------------------------------------------------------------------
// SvelteExtension --- the Svelte analogue of @lexical/react's ReactExtension.
//
// ReactExtension exposes a `decorators` channel: "mount this JSX somewhere
// inside the editor's React context; where exactly is not your business."
// React's own `createPortal(vnode, host, key)` then does the location-specific
// half, and --- crucially --- keeps the component subtree ALIVE while React
// moves its rendered DOM to a different host, because the subtree's identity is
// the (parent fiber, key) pair, not the host element.
//
// Svelte 5's `mount(Component, { target })` has no such indirection: the mounted
// instance is bound to the target it was given at mount time. There is no
// `setTarget`, and `unmount()` destroys state. So the naive port -- "mount into
// the host Lexical gives me" -- loses component state every time Lexical's
// reconciler rebuilds that host (createDOM re-runs on node replacement, on
// $applyNodeReplacement, on undo/redo restoring a destroyed key, ...).
//
// The workaround this spike validates: mount ONCE into a detached, spike-owned
// <span> that we keep forever, and then re-PARENT that span into whatever host
// Lexical currently offers. Moving a DOM node with appendChild preserves the
// node (and therefore every Svelte effect/binding pointing at it) --- the
// component never learns it moved. That is exactly the invariant createPortal
// buys you in React, obtained here with two lines of DOM instead of a
// reconciler feature.
// -----------------------------------------------------------------------------

import { mount, unmount, type Component } from "svelte";
import type { LexicalEditor, NodeKey } from "lexical";

export interface DecoratorSpec<Props extends Record<string, unknown>> {
    /** The Svelte component to mount per node. */
    component: Component<Props>;
    /** Build the props for a given NodeKey. Called once, at mount. */
    props: (key: NodeKey) => Props;
    /**
     * Given the node's top-level element, find the element the decorator should
     * live inside. Return null when the node has no usable host yet --- e.g. a
     * node replacement swapped createDOM out from under us.
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
 *
 * This is the whole seam. Everything else in the port is framework-agnostic.
 */
export function registerSvelteDecorator<Props extends Record<string, unknown>>(
    editor: LexicalEditor,
    // The node class, as `registerMutationListener` wants it.
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    nodeClass: any,
    spec: DecoratorSpec<Props>,
): () => void {
    const entries = new Map<NodeKey, Entry>();

    // Instrumentation for the spike UI: how many times did we mount vs. merely
    // re-parent an existing slot? A healthy run has mounts == distinct chips.
    const stats = getSeamStats();

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
            // into it, THEN attach. Mounting into a detached node is fine ---
            // Svelte does not require the target to be in the document.
            const slot = document.createElement("span");
            slot.dataset.svelteSlot = key;
            const instance = mount(spec.component, {
                target: slot,
                props: spec.props(key),
            }) as Record<string, unknown>;
            entry = { slot, instance, host: null };
            entries.set(key, entry);
            stats.mounts++;
        }

        // The load-bearing line. If Lexical rebuilt the host DOM, the slot is
        // now orphaned (or parked in the dead host); appendChild MOVES it into
        // the live one. The component instance, its $state, its effects, its
        // event listeners and any focus inside it all survive, because the node
        // itself was never destroyed --- only re-parented.
        if (entry.host !== host || entry.slot.parentElement !== host) {
            host.append(entry.slot);
            if (entry.host !== null && entry.host !== host) {
                stats.reparents++;
            }
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
        stats.unmounts++;
    };

    const unregisterMutations = editor.registerMutationListener(nodeClass, mutations => {
        for (const [key, kind] of mutations) {
            if (kind === "destroyed") {
                drop(key);
            } else {
                // "created" AND "updated" both run through ensure(). React's
                // portal version skips "updated"; we must not, because an
                // update is precisely when Lexical may have rebuilt the host.
                ensure(key);
            }
        }
    });

    // A mutation listener alone is not enough. Lexical rebuilds host DOM in
    // situations that produce no mutation record for the node in question ---
    // most obviously when an ANCESTOR is replaced, which re-runs createDOM on
    // the whole subtree. So we also sweep on every update: cheap, since it is
    // a getElementByKey per live decorator.
    const unregisterUpdates = editor.registerUpdateListener(() => {
        for (const key of [...entries.keys()]) {
            const element = editor.getElementByKey(key);
            if (!element) {
                // Node still exists in state but has no DOM (collapsed, off-
                // screen). Leave the entry alone; the next update re-homes it.
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

// ---- Spike instrumentation ---------------------------------------------------
// A module-level rune object: the .svelte.ts extension is what makes `$state`
// legal outside a component. Reading `seamStats.mounts` in a .svelte template
// is reactive with no subscription ceremony.
export const seamStats = $state({ mounts: 0, unmounts: 0, reparents: 0 });

function getSeamStats() {
    return seamStats;
}
