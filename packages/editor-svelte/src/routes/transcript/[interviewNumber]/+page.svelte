<script lang="ts">
    // Port of packages/editor/src/routes/transcript/$interviewNumber.tsx.
    //
    // `LexicalExtensionComposer` (React) memoises the editor on the extension's
    // identity and disposes the old one when it changes -- that is what the
    // source's `useMemo(..., [interviewUid])` bought it. `{#key interviewUid}`
    // is the direct Svelte analogue: it destroys and recreates everything
    // inside the block, including the editor host component below, whenever
    // interviewUid changes.
    import { browser } from "$app/environment";
    import { client } from "$lib/urql";
    import { createPlayhead } from "$lib/playhead.svelte";
    import { defineAuohpEditorExtension } from "$lib/editor/editor";
    import { SEARCH_STATEMENTS_QUERY } from "$lib/editor/search-interview/queries";
    import type { SearchStatementsQuery, SearchStatementsQueryVariables } from "$lib/editor/search-interview/__generated__/queries.gql";
    import {
        EDIT_STATEMENT_MUTATION,
        CREATE_STATEMENT_MUTATION,
        DESTROY_STATEMENT_MUTATION,
    } from "./queries";
    import type {
        EditStatementMutation,
        EditStatementMutationVariables,
        CreateStatementMutation,
        CreateStatementMutationVariables,
        DestroyStatementMutation,
        DestroyStatementMutationVariables,
    } from "./__generated__/queries.gql";
    import EditorHost from "./EditorHost.svelte";
    import type { PageProps } from "./$types";

    const { VITE_AUOHP_API_URI: AUOHP_API_URI } = import.meta.env;

    let { data }: PageProps = $props();

    let interviewUid = $derived(data.transcript?.interview?.uid ?? "");
    let interviewName = $derived(data.header?.interview?.interviewee?.name ?? "Unknown Interviewee");
    let statements = $derived(data.transcript?.interview?.transcript?.statements ?? []);
    let videoUri = $derived(data.transcript?.interview?.videos?.[0]?.uri);
    let statementHash = $state<string | undefined>(undefined);

    let player = $state<HTMLVideoElement | null>(null);

    // Plain async functions closing over the urql client -- the natural
    // equivalent of Apollo's useMutation executors, and what
    // PersistenceExtension's config expects (see PLAN.md sec 3.4).
    async function editStatement (variables: EditStatementMutationVariables) {
        const result = await client
            .mutation<EditStatementMutation, EditStatementMutationVariables>(EDIT_STATEMENT_MUTATION, variables)
            .toPromise();
        statementHash = result.data?.editStatement.newHash;
        return result;
    }

    async function createStatement (variables: CreateStatementMutationVariables) {
        return client
            .mutation<CreateStatementMutation, CreateStatementMutationVariables>(CREATE_STATEMENT_MUTATION, variables)
            .toPromise();
    }

    async function destroyStatement (variables: DestroyStatementMutationVariables) {
        return client
            .mutation<DestroyStatementMutation, DestroyStatementMutationVariables>(DESTROY_STATEMENT_MUTATION, variables)
            .toPromise();
    }

    async function searchStatements (variables: SearchStatementsQueryVariables) {
        return client
            .query<SearchStatementsQuery, SearchStatementsQueryVariables>(SEARCH_STATEMENTS_QUERY, variables)
            .toPromise();
    }
</script>

<svelte:head>
    <title>#{data.interviewNumber} - {interviewName} | AUOHP Editor</title>
</svelte:head>

<div class="page">
    <div class="video-container">
        {#if videoUri}
            <video bind:this={player} controls crossorigin="anonymous">
                <source src={videoUri} type="video/mp4" />
                {#key statementHash}
                    <track
                        default
                        kind="captions"
                        src="{AUOHP_API_URI}/interview/{data.interviewNumber}/vtt"
                        srclang="en"
                        label="English"
                    />
                {/key}
            </video>
        {/if}
    </div>

    <div class="editor-container">
        {#if browser}
            {#key interviewUid}
                <!--
                    The playhead is created HERE, inside the {#key} block,
                    alongside the editor extension it's passed to -- not
                    hoisted above it. One playhead per interview, destroyed
                    and rebuilt when the interview changes.

                    This also fixes a live bug in the source rather than just
                    tidying it up: `useVideoSync` there is an unconditional
                    effect on the module-singleton `seek`, so switching
                    interviews leaves `seek` holding the PREVIOUS interview's
                    position and fires it against the new <video> the moment
                    the new route mounts -- masked today only by the video's
                    own load sequence, not by anything deliberate. Scoping the
                    playhead's lifetime to interviewUid removes the stale
                    value instead of racing it: a fresh playhead starts at 0,
                    and there is no sense in which a timestamp from one
                    interview is meaningful against another's video anyway.
                -->
                {@const playhead = createPlayhead()}
                {@const extension = defineAuohpEditorExtension({
                    statements,
                    playhead,
                    editStatement,
                    createStatement,
                    destroyStatement,
                    searchStatements,
                    interviewUid,
                })}
                <EditorHost
                    {extension}
                    {player}
                    {playhead}
                />
            {/key}
        {/if}
    </div>
</div>

<style>
    /* Statement wrapper, its non-editable chrome column, and the editable
       content element -- see StatementNode.createDOM / getDOMSlot. Global
       because the class names are stamped by Lexical's own DOM building, not
       by any Svelte component that could own a scoped <style>. */
    :global(.auohp-statement) {
        display: flex;
        gap: 0.75rem;
        align-items: flex-start;
        padding: 0.25rem 0;
    }

    :global(.auohp-statement__chrome) {
        /* Seeking is a click on this column specifically (see
           StatementSeekExtension), so the chrome has to look clickable --
           otherwise the only way to discover the gesture is by accident. */
        cursor: pointer;
        user-select: none;

        display: flex;
        flex-direction: column;
        flex-shrink: 0;

        min-width: 6rem;

        font-family: monospace;
        font-size: 0.75rem;
        color: #888;

        transition: color 0.12s ease;
    }

    :global(.auohp-statement__chrome:hover) {
        color: #333;
    }

    :global(.auohp-statement__content) {
        flex: 1;
    }

    .page {
        overflow: hidden;
        display: grid;
        grid-template-rows: 1fr auto;

        width: 100vw;
        height: 100vh;
    }

    .video-container {
        display: flex;
        align-items: center;
        justify-content: center;
    }

    .editor-container {
        position: relative;
        overflow-y: auto;
        background: var(--spectrum-gray-75, #f8f8f8);
        padding: 12px;
    }
</style>
