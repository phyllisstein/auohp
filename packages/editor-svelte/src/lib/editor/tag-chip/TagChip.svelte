<script lang="ts">
    // The chip decorator, mounted per NodeKey by registerSvelteDecorator. Pure
    // function of the editor -- reads its own ids back out of EditorState
    // rather than keeping a second copy of them.
    //
    // `editor` comes in as a prop, not via context: mount() starts a new root
    // and Svelte context does not cross it (see svelte-decorator.ts).

    import type { LexicalEditor } from "lexical";
    import { $isTagChipNode as isTagChipNode } from "./TagChipNode";

    let { editor, nodeKey }: { editor: LexicalEditor; nodeKey: string } = $props();

    let ids = $state<string[]>([]);

    const readIds = () => {
        editor.read(() => {
            const node = editor.getEditorState()._nodeMap.get(nodeKey);
            ids = isTagChipNode(node) ? node.getIDs() : [];
        });
    };

    $effect(() => {
        readIds();
        return editor.registerUpdateListener(readIds);
    });
</script>

<span class="tag-chip__badge" data-node-key={nodeKey} title={ids.join(", ")}>
    <span class="tag-chip__dot"></span>
</span>

<style>
    .tag-chip__badge {
        display: inline-flex;
        align-items: center;
        vertical-align: baseline;
        user-select: none;
    }
    .tag-chip__dot {
        width: 0.55em;
        height: 0.55em;
        border-radius: 50%;
        background: var(--auohp-tag-chip-color, #b36);
        display: inline-block;
    }
</style>
