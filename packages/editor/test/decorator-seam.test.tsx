import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { act, useEffect, type JSX } from "react";
import { createRoot, type Root } from "react-dom/client";
import { $getNodeByKey, $getRoot, createEditor, type LexicalEditor, type NodeKey } from "lexical";
import { createLexicalComposerContext, LexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { useNodeDecorators, type ResolveHost } from "~/lexical/react-decorator";
import { $createDecoratorHostNode, DECORATOR_HOST_BADGE_CLASS, DecoratorHostNode } from "./fixtures/DecoratorHostNode";

// The four-gesture table from the Svelte port's seam test, asserted against
// mount counts. A face that gets remounted (rather than kept alive while its
// slot is re-parented) shows up as its count climbing past 1; the seam's job is
// to keep it at 1 across every gesture except a genuine node replacement.
//
// The "updated" rebuild is the gesture the per-feature portals this seam
// replaced got wrong: they skipped "updated" mutations, so the face stayed in
// the dead badge. Portalling into the new badge instead would have remounted
// it, because React identifies a portal's subtree by its container.

declare global {
    var IS_REACT_ACT_ENVIRONMENT: boolean | undefined;
}
globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const FIXTURE = "[data-testid='decorator-fixture']";

const resolveHost: ResolveHost = element =>
    element.querySelector<HTMLElement>(`:scope > .${ DECORATOR_HOST_BADGE_CLASS }`);

function Fixture ({ nodeKey, mountCounts }: { nodeKey: NodeKey; mountCounts: Map<NodeKey, number> }): JSX.Element {
    useEffect(() => {
        mountCounts.set(nodeKey, (mountCounts.get(nodeKey) ?? 0) + 1);
    }, [nodeKey, mountCounts]);

    return <span data-testid="decorator-fixture" />;
}

function Portals ({ mountCounts }: { mountCounts: Map<NodeKey, number> }): JSX.Element {
    return useNodeDecorators(DecoratorHostNode, resolveHost, key => <Fixture nodeKey={ key } mountCounts={ mountCounts } />);
}

describe("useNodeDecorators", () => {
    let container: HTMLElement;
    let reactContainer: HTMLElement;
    let editor: LexicalEditor;
    let root: Root;
    let mountCounts: Map<NodeKey, number>;

    beforeEach(async () => {
        container = document.createElement("div");
        reactContainer = document.createElement("div");
        document.body.append(container, reactContainer);

        editor = createEditor({ nodes: [DecoratorHostNode], onError: error => {
            throw error;
        } });
        editor.setRootElement(container);

        mountCounts = new Map();
        root = createRoot(reactContainer);
        await act(async () => {
            root.render(
                <LexicalComposerContext.Provider value={ [editor, createLexicalComposerContext(null, null)] }>
                    <Portals mountCounts={ mountCounts } />
                </LexicalComposerContext.Provider>,
            );
        });
    });

    afterEach(async () => {
        await act(async () => root.unmount());
        container.remove();
        reactContainer.remove();
    });

    const update = (fn: () => void) => act(async () => {
        editor.update(fn, { discrete: true });
    });

    const createHostNode = async (forceRebuild = false): Promise<NodeKey> => {
        let key = "";
        await update(() => {
            const node = $createDecoratorHostNode(forceRebuild);
            $getRoot().append(node);
            key = node.getKey();
        });
        return key;
    };

    it("keeps the face alive across an 'updated' mutation that rebuilds the element", async () => {
        const key = await createHostNode(true);
        expect(mountCounts.get(key)).toBe(1);
        const badgeBefore = container.querySelector(`.${ DECORATOR_HOST_BADGE_CLASS }`);
        expect(badgeBefore?.querySelector(FIXTURE)).not.toBeNull();

        await update(() => {
            $getNodeByKey(key)?.markDirty();
        });

        const badgeAfter = container.querySelector(`.${ DECORATOR_HOST_BADGE_CLASS }`);
        // The rebuild really happened: the badge is a different element.
        expect(badgeAfter).not.toBe(badgeBefore);
        // The face followed it into the new badge without remounting.
        expect(badgeAfter?.querySelector(FIXTURE)).not.toBeNull();
        expect(mountCounts.get(key)).toBe(1);
    });

    it("keeps faces alive across a reorder", async () => {
        const keyA = await createHostNode();
        const keyB = await createHostNode();

        await update(() => {
            const nodeA = $getNodeByKey(keyA);
            const nodeB = $getNodeByKey(keyB);
            if (nodeA && nodeB) {
                nodeB.insertBefore(nodeA);
            }
        });

        expect(mountCounts.get(keyA)).toBe(1);
        expect(mountCounts.get(keyB)).toBe(1);
        expect(container.querySelectorAll(FIXTURE)).toHaveLength(2);
    });

    it("keeps the face alive across root detach/reattach", async () => {
        const key = await createHostNode();

        await act(async () => editor.setRootElement(null));
        expect(mountCounts.get(key)).toBe(1);

        await act(async () => editor.setRootElement(container));

        expect(mountCounts.get(key)).toBe(1);
        const badge = container.querySelector(`.${ DECORATOR_HOST_BADGE_CLASS }`);
        expect(badge?.querySelector(FIXTURE)).not.toBeNull();
    });

    it("mounts a fresh face on a genuine node replacement (new NodeKey)", async () => {
        const key = await createHostNode();

        await update(() => {
            $getNodeByKey(key)?.replace($createDecoratorHostNode());
        });

        // The old key's face was torn down; its count never advanced.
        expect(mountCounts.get(key)).toBe(1);

        const newKeys = [...mountCounts.keys()].filter(k => k !== key);
        expect(newKeys).toHaveLength(1);
        expect(mountCounts.get(newKeys[0])).toBe(1);
        expect(container.querySelectorAll(FIXTURE)).toHaveLength(1);
    });
});
