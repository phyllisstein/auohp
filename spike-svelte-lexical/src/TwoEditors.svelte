<script lang="ts">
    // -------------------------------------------------------------------------
    // Second-editor-instance probe.
    //
    // Two live Lexical editors on one page, whose extension graphs BOTH list the
    // same extension objects (TagChipExtension, and a probe extension carrying
    // `namedSignals`). Questions:
    //
    //   - does `buildEditorFromExtensions` dedupe a shared extension to ONE
    //     instance across editors, or build one per editor?
    //   - is `namedSignals` state therefore shared or independent?
    //   - do NodeKeys, which are only unique WITHIN an editor, collide in the
    //     decorator seam's host map?
    // -------------------------------------------------------------------------

    import { onMount } from "svelte";
    import {
        $createParagraphNode as createParagraphNode,
        $createTextNode as createTextNode,
        $getRoot as getRoot,
        defineExtension,
    } from "lexical";
    import {
        buildEditorFromExtensions,
        getExtensionDependencyFromEditor,
        namedSignals,
    } from "@lexical/extension";
    import { RichTextExtension } from "@lexical/rich-text";

    import { TagChipExtension } from "./extensions";

    // How many times did `build()` actually run? If the module-level counter
    // reaches 2 with two editors, the extension is re-instantiated PER EDITOR
    // and its signals are independent. If it stays 1, they are shared.
    let buildCount = 0;

    const ProbeExtension = defineExtension({
        name: "@auohp/probe",
        build: () => {
            buildCount += 1;
            return namedSignals({ hits: 0 });
        },
    });

    let elA: HTMLDivElement;
    let elB: HTMLDivElement;
    let report = $state("(not run)");

    const makeEditor = (host: HTMLDivElement, seed: string) => {
        const editor = buildEditorFromExtensions(
            defineExtension({
                name: `@auohp/probe-root-${seed}`,
                dependencies: [RichTextExtension, TagChipExtension, ProbeExtension],
                namespace: `probe-${seed}`,
                $initialEditorState: () => {
                    const root = getRoot();
                    root.clear();
                    const paragraph = createParagraphNode();
                    paragraph.append(createTextNode(seed));
                    root.append(paragraph);
                },
                onError: (error: Error) => console.error(error),
            }),
        );
        editor.setRootElement(host);
        return editor;
    };

    onMount(() => {
        const before = buildCount;
        const a = makeEditor(elA, "Editor A: Larry Kramer founded ACT UP.");
        const b = makeEditor(elB, "Editor B: the FDA action was a turning point.");

        // Reach the probe signals through each editor's own dependency graph.
        type Hits = { hits: { value: number; peek(): number } };
        let signalsShared: string;
        try {
            const outA = getExtensionDependencyFromEditor(a, ProbeExtension).output as Hits;
            const outB = getExtensionDependencyFromEditor(b, ProbeExtension).output as Hits;
            outA.hits.value = 41;
            outB.hits.value = 7;
            signalsShared =
                outA.hits.peek() === outB.hits.peek()
                    ? `SHARED (both now read ${outA.hits.peek()})`
                    : `INDEPENDENT (A=${outA.hits.peek()}, B=${outB.hits.peek()})`;
        } catch (error) {
            signalsShared = `lookup threw: ${(error as Error).message}`;
        }

        // NodeKey collision check: both editors seeded identically-shaped
        // documents, so their key spaces overlap by construction.
        const keysA = a.getEditorState().read(() => [...getRoot().getChildrenKeys()]);
        const keysB = b.getEditorState().read(() => [...getRoot().getChildrenKeys()]);

        report =
            `build() ran ${buildCount - before}x for 2 editors sharing ProbeExtension` +
            ` -> namedSignals are ${signalsShared}` +
            ` | top-level NodeKeys A=[${keysA}] B=[${keysB}] (overlap: ${
                keysA.some(k => keysB.includes(k)) ? "YES" : "no"
            })`;

        return () => {
            a.dispose();
            b.dispose();
        };
    });

</script>

<section class="two">
    <h2>Second editor instance probe</h2>
    <p class="report">{report}</p>
    <div class="pair">
        <div bind:this={elA} contenteditable="true" class="mini" spellcheck="false"></div>
        <div bind:this={elB} contenteditable="true" class="mini" spellcheck="false"></div>
    </div>
</section>

<style>
    .two {
        margin-top: 2rem;
        border-top: 1px solid #eee;
        padding-top: 1rem;
    }
    h2 {
        font-size: 1rem;
    }
    .report {
        font: 12px/1.6 ui-monospace, monospace;
        background: #f6f6f8;
        padding: 0.5rem 0.7rem;
        border-radius: 6px;
    }
    .pair {
        display: grid;
        grid-template-columns: 1fr 1fr;
        gap: 0.75rem;
    }
    .mini {
        border: 1px solid #ddd;
        border-radius: 6px;
        padding: 0.5rem 0.7rem;
        outline: none;
        min-height: 3rem;
        background: #fff;
    }
</style>
