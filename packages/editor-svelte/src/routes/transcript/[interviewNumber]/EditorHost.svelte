<script lang="ts">
    // Builds the Lexical editor from the composed extension and mounts it
    // into a contenteditable div. `LexicalExtensionComposer`'s job (memoise
    // the editor on the extension's identity, dispose the old one on
    // teardown) is done for us here by Svelte's own lifecycle: this whole
    // component is destroyed and recreated by the parent's {#key
    // interviewUid} block, so `$effect`'s cleanup on `extension` is a normal
    // teardown, not a special case.
    //
    // The editor does not server-render (`{#if browser}` in the parent
    // guards this component's existence entirely), and that is a property of
    // Lexical's architecture, not a missing configuration: it is
    // UNCONTROLLED, the DOM is authoritative, and decorators are mounted into
    // elements the editor itself creates at runtime
    // (`editor.getElementByKey`). There is no DOM during SSR, so there is
    // nowhere for a decorator host to exist.
    import { buildEditorFromExtensions, type AnyLexicalExtension } from "@lexical/extension";
    import { untrack } from "svelte";
    import type { Playhead } from "$lib/playhead.svelte";

    let {
        extension,
        player,
        playhead,
    }: {
        extension: AnyLexicalExtension;
        player: HTMLVideoElement | null;
        playhead: Playhead;
    } = $props();

    let contentEditable = $state<HTMLElement>();

    $effect(() => {
        const editor = buildEditorFromExtensions(extension);
        // By the time an effect body runs, the component has already mounted
        // and bind:this has already written contentEditable -- untrack keeps
        // this effect keyed on `extension` alone, so a later contentEditable
        // write (there won't be one; bind:this only fires once per mount)
        // can't trigger a second build-and-dispose of the whole editor.
        untrack(() => {
            if (contentEditable) {
                editor.setRootElement(contentEditable);
            }
        });
        return () => editor.dispose();
    });

    // Drives the <video> from this editor instance's playhead. Reading
    // `playhead.seek` here makes the effect reactive to it; `player` comes
    // from the parent's bind:this and is reactive too, so a video that
    // mounts after the first seek still receives it.
    $effect(() => {
        const seek = playhead.seek;
        if (player) {
            player.currentTime = seek;
        }
    });

    // The reverse direction: the video's own playback position feeds
    // UpdateTimestampExtension's reads. A plain DOM listener rather than an
    // `ontimeupdate` prop on the <video>, since the element itself lives
    // outside this component (and outside the {#key interviewUid} block
    // this component is scoped to) -- it survives interview switches, this
    // component does not.
    $effect(() => {
        if (!player) {
            return;
        }
        const onTimeUpdate = () => {
            playhead.timestamp = player!.currentTime;
        };
        player.addEventListener("timeupdate", onTimeUpdate);
        return () => player!.removeEventListener("timeupdate", onTimeUpdate);
    });
</script>

<div
    bind:this={contentEditable}
    class="auohp-editor"
    contenteditable="true"
    spellcheck="false"
></div>

<style>
    .auohp-editor {
        outline: none;
    }
</style>
