<script lang="ts">
    import { onMount } from "svelte";
    import {
        $createParagraphNode as createParagraphNode,
        $createTextNode as createTextNode,
        $getRoot as getRoot,
        $getSelection as getSelection,
        $isRangeSelection as isRangeSelection,
        $nodesOfType as nodesOfType,
        defineExtension,
    } from "lexical";
    import { buildEditorFromExtensions } from "@lexical/extension";
    import { RichTextExtension } from "@lexical/rich-text";
    import { HistoryExtension } from "@lexical/history";

    import { INSERT_TAG_CHIP_COMMAND } from "./commands";
    import { TagChipExtension } from "./extensions";
    import { $createTagChipNode as createTagChipNode, TagChipNode } from "./nodes";
    import { seamStats } from "./svelte-extension.svelte";
    import { cyclePalette, tagPalette } from "./tag-signals.svelte";
    import { setEditor } from "./editor-context";
    import TwoEditors from "./TwoEditors.svelte";

    let contentEl: HTMLDivElement;
    let editor: ReturnType<typeof buildEditorFromExtensions>;
    let log = $state<string[]>([]);

    const note = (message: string) => {
        log = [`${new Date().toLocaleTimeString()}  ${message}`, ...log].slice(0, 12);
    };

    onMount(() => {
        editor = buildEditorFromExtensions(
            defineExtension({
                name: "@auohp/spike-root",
                dependencies: [RichTextExtension, HistoryExtension, TagChipExtension],
                namespace: "auohp-spike",
                $initialEditorState: () => {
                    const root = getRoot();
                    root.clear();
                    for (const text of [
                        "Larry Kramer founded ACT UP in nineteen eighty-seven.",
                        "The demonstration at the FDA was a turning point.",
                    ]) {
                        const paragraph = createParagraphNode();
                        paragraph.append(createTextNode(text));
                        root.append(paragraph);
                    }
                },
                onError: (error: Error) => console.error(error),
            }),
        );

        setEditor(editor);
        editor.setRootElement(contentEl);

        return () => editor.dispose();
    });

    const tag = () => {
        editor.update(() => {
            const selection = getSelection();
            if (!isRangeSelection(selection) || selection.isCollapsed()) {
                note("select some text first");
                return;
            }
            editor.dispatchCommand(INSERT_TAG_CHIP_COMMAND, `tag-${Math.random().toString(36).slice(2, 7)}`);
            note("tagged selection");
        });
    };

    // --- The reconciler-abuse buttons ------------------------------------------

    // Mutate every chip's __ids. This is the ordinary "updated" mutation: the
    // node keeps its key and its DOM element, MarkNode.updateDOM returns false,
    // and the Svelte slot is untouched. The chip's own update listener re-reads
    // the ids, so the tooltip changes without a re-mount.
    const churnIds = () => {
        editor.update(() => {
            for (const node of nodesOfType(TagChipNode)) {
                node.setIDs([...node.getIDs(), `churn-${Math.floor(Math.random() * 90 + 10)}`]);
            }
            note("mutated every chip's ids (updated mutation, no DOM rebuild)");
        });
    };

    // Detach and reattach the whole root element: every DOM node under the
    // editor is thrown away and rebuilt from EditorState. This is the nuclear
    // reconciler pass.
    const rebuildRoot = () => {
        editor.setRootElement(null);
        editor.setRootElement(contentEl);
        note("root element detached + reattached (full DOM rebuild)");
    };

    // Move a whole paragraph, which re-parents every descendant's DOM.
    const swapParagraphs = () => {
        editor.update(() => {
            const [first, second] = getRoot().getChildren();
            if (first && second) {
                second.insertBefore(first);
                note("swapped paragraph order");
            }
        });
    };

    // The one case that MUST re-mount: the node is genuinely destroyed and a
    // different NodeKey takes its place. There is no state to preserve here ---
    // React's portal would lose it too, since the portal is keyed by NodeKey.
    // Included so the counters distinguish "seam is broken" from "the document
    // legitimately replaced the thing".
    const replaceNodes = () => {
        editor.update(() => {
            for (const node of nodesOfType(TagChipNode)) {
                const replacement = createTagChipNode(node.getIDs());
                for (const child of node.getChildren()) {
                    replacement.append(child);
                }
                node.replace(replacement);
            }
            note("replaced every chip node (new NodeKey --- must re-mount)");
        });
    };
</script>

<main>
    <h1>Svelte × Lexical decorator seam</h1>
    <p class="lede">
        Each chip mounts a Svelte component into a <code>setDOMUnmanaged</code> badge span.
        The chip shows a live seconds-alive counter held in the component's own
        <code>$state</code>. <strong>If the counter resets, the seam re-mounted and lost state.</strong>
        If it keeps climbing through every button below, the seam works.
    </p>

    <div class="toolbar">
        <button onclick={tag}>Tag selection</button>
        <button onclick={churnIds}>Mutate chip ids</button>
        <button onclick={swapParagraphs}>Swap paragraphs</button>
        <button onclick={rebuildRoot}>Rebuild whole root DOM</button>
        <button onclick={replaceNodes}>Replace chip nodes (new keys)</button>
        <button onclick={cyclePalette}>
            Cycle palette (<span style:color={tagPalette.color}>{tagPalette.label}</span>)
        </button>
    </div>

    <div class="editor-shell">
        <div bind:this={contentEl} contenteditable="true" class="editor" spellcheck="false"></div>
    </div>

    <section class="stats">
        <div><strong>{seamStats.mounts}</strong> mounts</div>
        <div><strong>{seamStats.reparents}</strong> re-parents (host moved, state kept)</div>
        <div><strong>{seamStats.unmounts}</strong> unmounts</div>
    </section>

    <TwoEditors />

    <ul class="log">
        {#each log as line (line)}
            <li>{line}</li>
        {/each}
    </ul>
</main>

<style>
    :global(body) {
        font: 15px/1.55 ui-sans-serif, system-ui, sans-serif;
        margin: 0;
        padding: 2rem;
        max-width: 60rem;
        color: #1b1b1f;
    }
    h1 {
        font-size: 1.3rem;
        margin: 0 0 0.5rem;
    }
    .lede {
        color: #555;
        max-width: 46rem;
    }
    .toolbar {
        display: flex;
        gap: 0.5rem;
        flex-wrap: wrap;
        margin: 1rem 0;
    }
    button {
        font: inherit;
        padding: 0.35em 0.8em;
        border: 1px solid #ccc;
        border-radius: 6px;
        background: #fff;
        cursor: pointer;
    }
    button:hover {
        background: #f4f4f6;
    }
    .editor-shell {
        border: 1px solid #ddd;
        border-radius: 8px;
        padding: 0.75rem 1rem;
        background: #fff;
    }
    .editor {
        outline: none;
        min-height: 6rem;
    }
    .editor :global(p) {
        margin: 0 0 0.6em;
    }
    .editor :global(mark.auohp-tag-chip) {
        background: color-mix(in srgb, currentColor 8%, transparent);
        border-radius: 4px;
        padding: 0 0.15em;
        box-shadow: inset 0 -2px 0 rgba(0, 0, 0, 0.12);
    }
    .stats {
        display: flex;
        gap: 1.5rem;
        margin: 1rem 0 0.5rem;
        font-size: 0.85rem;
        color: #444;
    }
    .log {
        font: 12px/1.6 ui-monospace, monospace;
        color: #666;
        list-style: none;
        padding: 0;
    }
</style>
