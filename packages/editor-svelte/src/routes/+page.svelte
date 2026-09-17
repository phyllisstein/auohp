<script lang="ts">
// Port of packages/editor/src/routes/index.tsx and routes/transcript/index.tsx,
// collapsed to one route -- see MIGRATION.md's known-defects list. Both source
// files were near-duplicate interview-list pages; only one is needed here.
//
// The source's LinkComponent/BasicLinkComponent/StackLink machinery exists
// solely to route React Aria hover/press/focus-visible state into an S2
// style() macro className. A plain <a> with :hover/:focus-visible CSS does
// the same job with no framework underneath it.
import type { PageProps } from "./$types";

let { data }: PageProps = $props();
</script>

<svelte:head>
    <title>AUOHP Editor</title>
</svelte:head>

<div class="panel">
    <div class="content">
        <h1>Interviews</h1>
        <ul>
            {#each data.interviews as interview (interview.number)}
                <li>
                    <a href="/transcript/{interview.number}">
                        #{interview.number} - {interview.interviewee.name}
                    </a>
                </li>
            {/each}
        </ul>
        <h2>Search</h2>
        <a href="/search">Search</a>
    </div>
</div>

<style>
    .panel {
        background-color: var(--spectrum-gray-100, #f5f5f5);
        height: 100%;
        padding: var(--spectrum-spacing-300, 1.5rem);
        border-radius: var(--spectrum-spacing-75, 4px);
    }

    .content {
        width: max-content;
        height: max-content;
    }

    a {
        color: var(--spectrum-accent-color-800, #0d66d0);
        cursor: pointer;
        transition: color 0.12s ease;
    }

    a:hover {
        color: var(--spectrum-accent-color-900, #0a52ab);
    }

    a:focus-visible {
        outline: 2px solid var(--spectrum-accent-color-800, #0d66d0);
        outline-offset: 2px;
    }
</style>
