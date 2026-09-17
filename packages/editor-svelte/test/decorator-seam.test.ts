import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { flushSync } from "svelte";
import {
    $getRoot,
    $getNodeByKey,
    createEditor,
    type LexicalEditor,
    type NodeKey,
} from "lexical";

import { registerSvelteDecorator } from "../src/lib/editor/svelte-decorator";
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
// registerUpdateListener sweep in svelte-decorator.ts therefore has
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

    const createHostNode = (forceRebuild = false): NodeKey => {
        let key = "";
        editor.update(
            () => {
                const node = $createDecoratorHostNode(forceRebuild);
                $getRoot().append(node);
                key = node.getKey();
            },
            { discrete: true },
        );
        return key;
    };

    it("survives an 'updated' mutation that rebuilds the element", () => {
        // forceRebuild makes updateDOM() return true, so markDirty() below
        // produces a genuine "updated" record whose badge element is a new
        // DOM node -- not a no-op that would pass whether or not the
        // "updated" branch of the mutation listener runs at all.
        const key = createHostNode(true);
        flushSync();
        expect(mountCounts.get(key)).toBe(1);
        const badgeBefore = container.querySelector(`.${ DECORATOR_HOST_BADGE_CLASS }`);

        editor.update(
            () => {
                const node = $getNodeByKey(key);
                node?.markDirty();
            },
            { discrete: true },
        );
        flushSync();

        const badgeAfter = container.querySelector(`.${ DECORATOR_HOST_BADGE_CLASS }`);
        // The rebuild really happened: the badge is a different element.
        expect(badgeAfter).not.toBe(badgeBefore);
        // The seam re-parented the same slot into the new badge rather than
        // mounting a fresh instance.
        expect(mountCounts.get(key)).toBe(1);
        expect(badgeAfter?.querySelector("[data-testid='decorator-fixture']")).not.toBeNull();

        // What this does and doesn't prove: registerUpdateListener's sweep
        // (see the seam module) walks every live entry on every update,
        // unconditionally, which makes it a strict superset of this mutation
        // listener's "created"/"updated" handling in lexical 0.49.0. Verified
        // by mutation testing: breaking the "updated" branch above while
        // leaving the sweep in place still leaves this test green, because
        // the sweep's very next pass repairs the re-parent before any
        // assertion runs. This test can only show the seam as a whole
        // survives a rebuild -- it cannot isolate the mutation listener's
        // "updated" branch from the sweep. That isolation only holds with
        // the sweep disabled too, which is not the shipped configuration.
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
        // mutation observer and clears textContent directly before
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
