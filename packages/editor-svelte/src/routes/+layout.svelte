<script lang="ts">
import "@spectrum-web-components/styles/tokens-v2/index.css";
import "@spectrum-web-components/styles/tokens-v2/global-vars.css";
import "@spectrum-web-components/styles/tokens-v2/dark-vars.css";
import "@spectrum-web-components/styles/tokens-v2/medium-vars.css";
import "@spectrum-web-components/styles/tokens-v2/system-theme-bridge.css";
import "@spectrum-web-components/bundle/elements.js";

let { children } = $props();
</script>

{@render children()}

<style>
    :global(body) {
        margin: 0;
        background-color: var(--spectrum-gray-200);
        color: var(--spectrum-gray-900);
        font-family:
            "Adobe Clean",
            system-ui,
            -apple-system,
            BlinkMacSystemFont,
            "Segoe UI",
            Roboto,
            Oxygen,
            Ubuntu,
            Cantarell,
            "Open Sans",
            "Helvetica Neue",
            sans-serif;
        font-size: var(--spectrum-body-size-m, 112.5%);
    }
</style>
