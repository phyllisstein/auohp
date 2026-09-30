<script lang="ts">
    // Fixture for the decorator-seam test. Its only purpose is to expose
    // whether the component instance survived a host change: the count for
    // this nodeKey in `mountCounts` increments once, at mount, and stays put
    // across re-parents. If the seam ever re-mounts instead of moving the
    // DOM, a fresh instance bumps the count again. `mountCounts` is a plain
    // Map the test creates per-test and passes in via props -- not shared
    // state -- since there's nothing else outside the component to observe
    // mount timing from.

    import { onMount } from "svelte";

    let { nodeKey, mountCounts }: { nodeKey: string; mountCounts: Map<string, number> } = $props();

    onMount(() => {
        mountCounts.set(nodeKey, (mountCounts.get(nodeKey) ?? 0) + 1);
    });
</script>

<span data-testid="decorator-fixture" data-node-key={nodeKey}></span>
