<script>
  import { fly } from 'svelte/transition'

  let value = 0
</script>

<main>
  <slot />

  {#if value}
    <p>Value is truthy</p>
  {:else}
    <p>Value is falsy</p>
  {/if}

  Who likes {value} values?

  <input transition:fly={{ duration: 200, y: 2000 }} bind:value={value} {...fly} />
  <input transition:fly={{ duration: 200, y: 2000 }} bind:value={value} />
</main>
