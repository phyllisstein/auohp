<script lang="ts">
    // The mark's Svelte face, mounted per NodeKey by registerSvelteDecorator
    // into the unmanaged badge span SearchResultNode.createDOM builds.
    //
    // registerSvelteDecorator's `props: (key) => Props` is called ONCE, at
    // mount -- so a plain `focused: boolean` prop would never update. Instead
    // this component receives the shared reactive `output` object itself and
    // computes `focused` with `$derived`, which re-runs whenever the $state
    // proxy's fields it reads (resultKeys, focusedResult) change, regardless
    // of when the prop was handed in. `resultKeys[focusedResult] === nodeKey`
    // is the authority for "this is result N" -- never an index into
    // anything the decorator itself holds (see SearchOutput.resultKeys's own
    // doc comment for why: hosts arrive in mutation order, not document
    // order).

    import type { NodeKey } from "lexical";
    import type { SearchOutput } from "./search-output.svelte";

    let { output, nodeKey }: { output: SearchOutput; nodeKey: NodeKey } = $props();

    let focused = $derived(
        output.focusedResult !== null && output.resultKeys[output.focusedResult] === nodeKey,
    );

    let container: HTMLSpanElement | undefined = $state();

    $effect(() => {
        if (focused && container) {
            container.scrollIntoView({ behavior: "smooth", block: "center" });
        }
    });
</script>

<span bind:this={container} class="search-result__container" data-node-key={nodeKey}></span>
