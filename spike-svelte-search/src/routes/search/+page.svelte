<script lang="ts">
// Port of packages/editor/src/routes/search/results/route.tsx (ResultsPage).
// Bare $state reads -- Svelte compiler tracks them, re-renders on any write
// from the layout. (The original had a stray `A;` on line 11 -- a syntax
// error in the source. Dropped.)
// import "@spectrum-web-components/card/sp-card.js";
import { searchQuery } from "$lib/search.svelte";

// `null` = no search has run; `[]` = ran and matched nothing.
let hits = $derived(searchQuery.results?.search.statementText ?? null);
</script>

<div class="col">
    {#if searchQuery.loading}
        <p>Searching for &ldquo;{searchQuery.query}&rdquo;&hellip;</p>
    {:else if searchQuery.error}
        <p role="alert">Search failed: {searchQuery.error.message}</p>
    {:else if hits == null}
        <p>Enter a query to search the transcripts.</p>
    {:else if hits.length === 0}
        <p>No results found for &ldquo;{searchQuery.query}&rdquo;.</p>
    {:else}
        {#each hits as hit (hit.statement.uid)}
            <sp-card
                heading="Interview #{hit.interview.number} — {hit.interview
                    .interviewee.name}"
            >
                <div slot="description">
                    <p>{hit.statement.text}</p>
                    <p class="times">
                        Start time: {hit.statement.startTime} | End time: {hit
                            .statement.endTime}
                    </p>
                </div>
            </sp-card>
        {/each}
    {/if}
</div>

<style>
    .col {
        display: flex;
        flex-direction: column;
        gap: var(--spectrum-spacing-100, 1rem);
        width: 100%;
    }
    .times {
        font-style: italic;
        color: var(--spectrum-gray-700, #666);
    }
</style>
