<script lang="ts">
    // The chip's Svelte face. Like the React one it receives only a NodeKey and
    // reads everything else back out of EditorState --- a pure function of the
    // editor, not a second copy of it.
    //
    // The `mountedAt` / `ticks` state is deliberately LOCAL and deliberately
    // ugly-visible: it is the state that would be lost if the seam re-mounted
    // the component when Lexical rebuilt the host. If the counter keeps
    // climbing while chips get moved around by the reconciler, the seam works.

    import { onMount } from "svelte";
    import { getEditor } from "./editor-context";
    import { $isTagChipNode as isTagChipNode } from "./nodes";
    import { tagPalette } from "./tag-signals.svelte";

    let { nodeKey }: { nodeKey: string } = $props();

    const editor = getEditor();

    const mountedAt = Date.now();
    let ticks = $state(0);
    let ids = $state<string[]>([]);

    const readIds = () => {
        editor.read(() => {
            const node = editor.getEditorState()._nodeMap.get(nodeKey);
            ids = isTagChipNode(node as never) ? (node as never as { getIDs(): string[] }).getIDs() : [];
        });
    };

    onMount(() => {
        readIds();
        const timer = setInterval(() => (ticks += 1), 1000);
        const unregister = editor.registerUpdateListener(readIds);
        return () => {
            clearInterval(timer);
            unregister();
        };
    });

    // A bare rune read across a module boundary. Compare `useExtensionSignalValue`
    // in React, which needs a hook call per signal per component.
    const color = $derived(tagPalette.color);
</script>

<span
    class="tag-chip__container"
    data-node-key={nodeKey}
    style:--chip-color={color}
    title="key {nodeKey} / ids {ids.join(',')} / alive {ticks}s"
>
    <span class="tag-chip__dot"></span>
    <span class="tag-chip__age">{ticks}s</span>
</span>

<style>
    .tag-chip__container {
        display: inline-flex;
        align-items: center;
        gap: 0.2em;
        vertical-align: baseline;
        user-select: none;
    }
    .tag-chip__dot {
        width: 0.55em;
        height: 0.55em;
        border-radius: 50%;
        background: var(--chip-color, #b36);
        display: inline-block;
    }
    .tag-chip__age {
        font-size: 0.65em;
        font-variant-numeric: tabular-nums;
        color: var(--chip-color, #b36);
        opacity: 0.85;
    }
</style>
