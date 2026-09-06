<script lang="ts">
    // Fixture for the decorator-seam test. Its only purpose is to expose
    // whether the component instance survived a host change: `mountCount`
    // increments once, at mount, and stays put across re-parents. If the seam
    // ever re-mounts instead of moving the DOM, this resets to 1 again on a
    // fresh instance -- the test reads it off `slot.__mountCounts` (a global,
    // keyed by nodeKey) rather than the DOM, since there's nothing else
    // outside the component to observe it from.

    import { onMount } from "svelte";

    let { nodeKey, mountCounts }: { nodeKey: string; mountCounts: Map<string, number> } = $props();

    onMount(() => {
        mountCounts.set(nodeKey, (mountCounts.get(nodeKey) ?? 0) + 1);
    });
</script>

<span data-testid="decorator-fixture" data-node-key={nodeKey}></span>
