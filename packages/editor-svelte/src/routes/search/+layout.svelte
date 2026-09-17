<script lang="ts">
// Port of packages/editor/src/routes/search/route.tsx (SearchPage).
//
// Router leg: the original mounted /search/results as a child <Outlet/> and
// navigated to it with `mask: { to: "/search" }` -- URL stays /search, the
// results child renders from the in-memory signal. SvelteKit has no URL-mask
// primitive. Chosen equivalent: a nested layout (this file = the search bar
// chrome) wrapping /search/+page.svelte (the results view), which always
// renders and reads the shared $state. No navigation, no URL change at all.
// Since the results were never URL-restorable in the original either, this
// is behaviourally equivalent and simpler.

import { client } from "$lib/urql";
import { searchQuery } from "$lib/search.svelte";
import {
    SEARCH_ALL_STATEMENTS_QUERY,
    type SearchAllStatementsQuery,
    type SearchAllStatementsQueryVariables,
} from "$lib/graphql";

let { children } = $props();

async function runSearch () {
    // Quoted so the backend treats it as a phrase.
    const fragment = `"${ searchQuery.query }"`;

    searchQuery.loading = true;
    searchQuery.error = null;

    // urql resolves with { data, error } even on GraphQL errors (its default
    // == Apollo's errorPolicy:"all"). Svelte batches synchronous mutations
    // within a tick, so no `batch()` wrapper is needed.
    const result = await client
        .query<
        SearchAllStatementsQuery,
        SearchAllStatementsQueryVariables
    >(SEARCH_ALL_STATEMENTS_QUERY, { fragment })
        .toPromise();

    searchQuery.loading = false;
    searchQuery.error = result.error ?? null;
    searchQuery.results = result.data ?? null;
}

function onInput (event: Event) {
    searchQuery.query = (event.target as HTMLInputElement).value;
}

function onKeydown (event: KeyboardEvent) {
    if (event.key === "Enter") runSearch();
}
</script>

<section>
    <div class="search-bar">
        <sp-search
            class="grow"
            label="Search transcript"
            value={searchQuery.query}
            oninput={onInput}
            onkeydown={onKeydown}
        ></sp-search>
        <sp-button
            size="m"
            pending={searchQuery.loading || undefined}
            onclick={runSearch}
        >
            <sp-icon-search slot="icon"></sp-icon-search>
            Search
        </sp-button>
    </div>
    <div class="results">
        {@render children()}
    </div>
</section>

<style>
    /* Gen-1 Spectrum CSS custom properties, each with a plain fallback for when
	   the theme fragment hasn't loaded (e.g. SSR, first paint). */
    .search-bar,
    .results {
        background: var(--spectrum-gray-100, #f5f5f5);
        padding: var(--spectrum-spacing-200, 1rem);
        margin: var(--spectrum-spacing-200, 1rem);
        border-radius: var(--spectrum-spacing-75, 4px);
    }
    .search-bar {
        display: flex;
        flex-direction: row;
        gap: var(--spectrum-spacing-200, 1rem);
        align-items: center;
        justify-content: space-between;
    }
    .results {
        display: flex;
    }
    .grow {
        flex: 1;
    }
</style>
