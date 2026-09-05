<script lang="ts">
import "@spectrum-web-components/styles/tokens-v2/index.css";
import "@spectrum-web-components/styles/tokens-v2/global-vars.css";
import "@spectrum-web-components/styles/tokens-v2/dark-vars.css";
import "@spectrum-web-components/styles/tokens-v2/medium-vars.css";
import "@spectrum-web-components/styles/tokens-v2/system-theme-bridge.css";
import "@spectrum-web-components/bundle/elements.js";


let { children } = $props();
</script>

<!-- svelte-ignore: Svelte 5 writes `system` as a property, not an attribute.
     sp-theme's runtime check wants the attribute and warns; the property IS
     set and the theme delivers correctly. Setting it imperatively in onMount
     silences it (see below) but the warning is cosmetic. -->

{@render children()}
<button>Hello, button</button>

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
