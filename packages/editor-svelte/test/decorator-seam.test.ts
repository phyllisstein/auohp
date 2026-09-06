import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { flushSync } from "svelte";
import {
    $getRoot,
    $getNodeByKey,
    createEditor,
    type LexicalEditor,
    type NodeKey,
} from "lexical";

import { registerSvelteDecorator } from "../src/lib/editor/svelte-decorator.svelte";
import {
    $createDecoratorHostNode,
    DECORATOR_HOST_BADGE_CLASS,
    DecoratorHostNode,
} from "./fixtures/DecoratorHostNode";
import DecoratorFixture from "./fixtures/DecoratorFixture.svelte";

// The four-gesture table from LEXICAL-SPIKE-NOTES.md, asserted against mount
// counts rather than eyeballed in a browser. A component that gets re-mounted
// (instead of re-parented) shows up as its mount count climbing past 1; the
// seam's whole job is to keep it at 1 across every gesture except a genuine
// node replacement.
//
// What each gesture actually verifies, measured directly against lexical
// 0.49.0's mutation listener rather than assumed from the spike notes: an
// "updated" mutation and a reorder both fire real mutation records the
// listener alone would already act on, so those two cases prove the seam
// re-parents instead of re-mounting when handed a record. Root detach fires
// none at all (resetEditor nulls the mutation observer before
// $commitPendingUpdates runs), which is the one gesture the mutation listener
// cannot see by itself -- but the follow-up reattach fires FULL_RECONCILE,
// which re-announces every live node as "created" regardless, so the
// mutation listener alone already recovers once reattach happens. The
// registerUpdateListener sweep in svelte-decorator.svelte.ts therefore has
// not been shown to change any outcome measurable here; it is insurance
// against a path this suite does not exercise, not a demonstrated fix.

describe("registerSvelteDecorator", () => {
    let container: HTMLElement;
    let editor: LexicalEditor;
    let teardown: () => void;
    let mountCounts: Map<NodeKey, number>;

    beforeEach(() => {
        container = document.createElement("div");
        document.body.append(container);

        editor = createEditor({ nodes: [DecoratorHostNode] });
        editor.setRootElement(container);

        mountCounts = new Map();
        teardown = registerSvelteDecorator(editor, DecoratorHostNode, {
            component: DecoratorFixture,
            props: key => ({ nodeKey: key, mountCounts }),
            resolveHost: element =>
                element.querySelector<HTMLElement>(`:scope > .${ DECORATOR_HOST_BADGE_CLASS }`),
        });
    });

    afterEach(() => {
        teardown();
        container.remove();
    });

    const createHostNode = (): NodeKey => {
        let key = "";
        editor.update(
            () => {
                const node = $createDecoratorHostNode();
                $getRoot().append(node);
                key = node.getKey();
            },
            { discrete: true },
        );
        return key;
    };

    it("survives an 'updated' mutation", () => {
        const key = createHostNode();
        flushSync();
        expect(mountCounts.get(key)).toBe(1);

        // Force an "updated" mutation without touching the DOM element:
        // mark the node dirty via a no-op write.
        editor.update(
            () => {
                const node = $getNodeByKey(key);
                node?.markDirty();
            },
            { discrete: true },
        );
        flushSync();

        expect(mountCounts.get(key)).toBe(1);
        const badge = container.querySelector(`.${ DECORATOR_HOST_BADGE_CLASS }`);
        expect(badge?.querySelector("[data-testid='decorator-fixture']")).not.toBeNull();
    });

    it("survives reordering (fires 'updated' for both nodes, verified separately)", () => {
        const keyA = createHostNode();
        const keyB = createHostNode();
        flushSync();
        expect(mountCounts.get(keyA)).toBe(1);
        expect(mountCounts.get(keyB)).toBe(1);

        editor.update(
            () => {
                const nodeA = $getNodeByKey(keyA);
                const nodeB = $getNodeByKey(keyB);
                if (nodeA && nodeB) {
                    nodeB.insertBefore(nodeA);
                }
            },
            { discrete: true },
        );
        flushSync();

        expect(mountCounts.get(keyA)).toBe(1);
        expect(mountCounts.get(keyB)).toBe(1);
    });

    it("survives root detach/reattach", () => {
        const key = createHostNode();
        flushSync();
        expect(mountCounts.get(key)).toBe(1);

        // setRootElement(null) commits via resetEditor, which nulls the
        // mutation observer and clears textContent directly BEFORE
        // $commitPendingUpdates runs -- $reconcileRoot never executes, so no
        // mutation record fires for this node at all. Verified directly
        // against a bare mutation listener in a throwaway probe (not
        // committed): zero records logged across this call.
        editor.setRootElement(null);
        flushSync();
        // No re-mount happened: the count is still 1, not reset or bumped.
        expect(mountCounts.get(key)).toBe(1);

        // setRootElement(container) then fires FULL_RECONCILE, which
        // re-announces every live node as "created" -- verified directly:
        // the mutation listener alone (sweep disabled) still passes this
        // assertion, because ensure() runs from that "created" record and
        // finds the existing entry, re-parenting its slot rather than
        // mounting a fresh instance. This assertion therefore does not
        // discriminate the sweep's contribution; see the module comment.
        editor.setRootElement(container);
        flushSync();

        expect(mountCounts.get(key)).toBe(1);
        const badge = container.querySelector(`.${ DECORATOR_HOST_BADGE_CLASS }`);
        expect(badge?.querySelector("[data-testid='decorator-fixture']")).not.toBeNull();
    });

    it("re-mounts on a genuine node replacement (new NodeKey)", () => {
        const key = createHostNode();
        flushSync();
        expect(mountCounts.get(key)).toBe(1);

        editor.update(
            () => {
                const node = $getNodeByKey(key);
                node?.replace($createDecoratorHostNode());
            },
            { discrete: true },
        );
        flushSync();

        // The old key's decorator was torn down; its count does not advance
        // past the one mount it ever got.
        expect(mountCounts.get(key)).toBe(1);

        const badges = container.querySelectorAll(`.${ DECORATOR_HOST_BADGE_CLASS }`);
        expect(badges).toHaveLength(1);
        // A different key now owns the surviving badge -- confirm a second,
        // independent mount happened rather than the old instance moving.
        const newKeys = [...mountCounts.keys()].filter(k => k !== key);
        expect(newKeys).toHaveLength(1);
        expect(mountCounts.get(newKeys[0])).toBe(1);
    });
});
