<script lang="ts">
	// Spectrum Web Components Gen 1 (@spectrum-web-components/*@1.x). Each import
	// is side-effectful: it registers the custom element. Theme fragments
	// (system/color/scale) are separate side-effect imports too.
	import "@spectrum-web-components/theme/sp-theme.js";
	import "@spectrum-web-components/theme/scale-medium.js";
	import "@spectrum-web-components/theme/theme-light.js";
	import { onMount } from "svelte";

	let { children } = $props();

	let themeEl: HTMLElement;

	// Svelte 5 sets `system` on the custom element as a property (because
	// SpectrumElement declares an accessor for it), not as an attribute.
	// sp-theme's own validation reads the attribute and warns when it's
	// missing. Reflect it once mounted. `color`/`scale` land as attributes
	// on their own, so only `system` needs this.
	onMount(() => themeEl.setAttribute("system", "spectrum"));
</script>

<!-- svelte-ignore: Svelte 5 writes `system` as a property, not an attribute.
     sp-theme's runtime check wants the attribute and warns; the property IS
     set and the theme delivers correctly. Setting it imperatively in onMount
     silences it (see below) but the warning is cosmetic. -->
<sp-theme bind:this={themeEl} system="spectrum" color="light" scale="medium">
	{@render children()}
</sp-theme>

<style>
	:global(body) {
		margin: 0;
		/* Gen-1 Spectrum global font token, with a plain fallback. */
		font-family: var(--spectrum-sans-font-family-stack, system-ui, sans-serif);
	}
</style>
