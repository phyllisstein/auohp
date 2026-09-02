import { TextField } from "@react-spectrum/s2/TextField";
import SearchIcon from "@react-spectrum/s2/icons/Search";
import { createFileRoute, Outlet, useNavigate } from "@tanstack/react-router";
import { style } from "@react-spectrum/s2/style" with { type: "macro" };
import styled from "styled-components";
import { Button, Text } from "@react-spectrum/s2/Button";
import { batch } from "@preact/signals-react";
import { gql, type TypedDocumentNode } from "@apollo/client";
import type {
    SearchAllStatementsQuery,
    SearchAllStatementsQueryVariables,
} from "./__generated__/index.gql";
import { useLazyQuery } from "@apollo/client/react";
import { searchQuery } from "./-search-signal";

export const SEARCH_ALL_STATEMENTS_QUERY: TypedDocumentNode<
    SearchAllStatementsQuery,
    SearchAllStatementsQueryVariables
> = gql`
  query SearchAllStatements($fragment: String!) {
    search {
      statementText(fragment: $fragment) {
        statement {
          uid
          text
          startTime
          endTime
        }
        interview {
          uid
          number
          interviewee {
            uid
            name
          }
        }
      }
    }
  }
`;

const ResultsContainer = styled.div`
    display: flex;
`;

const SearchContainer = styled.div`
    display: flex;
    flex-direction: row;
    gap: 1rem;
    align-items: center;
    justify-content: space-between;
`;

export const Route = createFileRoute("/search")({
    component: SearchPage,
});

function SearchPage () {
    // `useLazyQuery` gives us an imperative trigger. We do not read its returned
    // `loading`/`data`/`error` --- the model is the single source of truth, and
    // `runSearch` resolves with a snapshot of both, so one await is enough.
    //
    // `errorPolicy: "all"` is load-bearing: under Apollo Client 4's default
    // ("none") an errored query *rejects* the promise instead of resolving with
    // `{ error }`, which would skip the batch below and strand `loading` at
    // true. With "all", errors arrive in the resolved result (with possibly
    // partial `data`) and we can commit both.
    const [runSearch] = useLazyQuery(SEARCH_ALL_STATEMENTS_QUERY, {
        fetchPolicy: "network-only",
        errorPolicy: "all",
    });

    const navigate = useNavigate();

    async function runSearchHandler () {
        // The fragment is quoted so the backend treats it as a phrase.
        const fragment = `"${ searchQuery.query.value }"`;

        searchQuery.loading.value = true;
        searchQuery.error.value = null;

        const { data, error } = await runSearch({ variables: { fragment } });

        // One commit: subscribers see loading flip off *with* the new
        // results/error already in place, never an inconsistent in-between.
        batch(() => {
            searchQuery.loading.value = false;
            searchQuery.error.value = error ?? null;
            searchQuery.results.value = data ?? null;
        });

        // Mount the results child (it renders from the signal), but mask the URL
        // back to `/search`. The results view isn't URL-restorable --- it lives
        // in `searchQuery`, an in-memory signal --- so a copied or reloaded link
        // should land on the clean search page, not a `/search/results` that
        // would show an empty state. `unmaskOnReload` stays false: a reload
        // resolves the masked `/search`, discarding the transient results.
        navigate({
            to: "/search/results",
            mask: { to: "/search" },
        });
    }

    return (
        <section>
            <SearchContainer
                className={ style({
                    backgroundColor: "layer-2",
                    height: "full",
                    padding: "text-to-control",
                    margin: "text-to-control",
                    borderRadius: "sm",
                }) }>
                <TextField
                    styles={ style({ width: "full" }) }
                    aria-label="Search transcript"
                    type="search"
                    enterKeyHint="search"
                    inputMode="search"
                    prefix={ <SearchIcon /> }
                    size="M"
                    value={ searchQuery.query.value }
                    onChange={ value => {
                        searchQuery.query.value = value;
                    } }
                    onKeyDown={ event => {
                        if (event.key === "Enter") {
                            runSearchHandler();
                        }
                    } } />
                <Button
                    variant="primary"
                    size="M"
                    isPending={ searchQuery.loading.value }
                    onPress={ runSearchHandler }>
                    <SearchIcon />
                    <Text>Search</Text>
                </Button>
            </SearchContainer>
            <ResultsContainer
                className={ style({
                    backgroundColor: "layer-2",
                    height: "full",
                    padding: "text-to-control",
                    margin: "text-to-control",
                    borderRadius: "sm",
                }) }>
                <Outlet />
            </ResultsContainer>
        </section>
    );
}
