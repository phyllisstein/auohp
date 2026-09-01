import { ProgressCircle } from "@react-spectrum/s2/ProgressCircle";
import { TextField } from "@react-spectrum/s2/TextField";
import SearchIcon from "@react-spectrum/s2/icons/Search";
import { createFileRoute, useRouterState, useRouter } from "@tanstack/react-router";
import { style } from "@react-spectrum/s2/style" with { type: "macro" };
import styled from "styled-components";
import { Button, Text } from "@react-spectrum/s2/Button";
import { gql, type TypedDocumentNode } from "@apollo/client";
import type {
    SearchAllStatementsQuery,
    SearchAllStatementsQueryVariables,
} from "./__generated__/search.gql";
import { useLazyQuery } from "@apollo/client/react";
import { useEffect, useState } from "react";

export const SEARCH_ALL_STATEMENTS_QUERY: TypedDocumentNode<SearchAllStatementsQuery, SearchAllStatementsQueryVariables> = gql`
    query SearchAllStatements(
        $fragment: String!
    ) {
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
    const [runSearch, { data, loading, error }] = useLazyQuery(SEARCH_ALL_STATEMENTS_QUERY, {
        fetchPolicy: "network-only",
    });
    const router = useRouter();
    const state = useRouterState();
    const [query, setQuery] = useState<string>(state?.location?.state?.query ?? "");
    console.log("SearchPage state:", state);

    useEffect(() => {
        if (state?.location?.state?.query && state.location.state.query !== query) {
            setQuery(state.location.state.query);
            runSearch({
                variables: {
                    fragment: `"${ state.location.state.query }"`,
                },
            });
        }
    }, [state?.location?.state?.query]);

    const spinner = (
        <ProgressCircle
            aria-label="Loading…"
            value={ 80 }
            isIndeterminate
            size="S"
            staticColor="white" />
    );

    async function runSearchHandler () {
        await runSearch({
            variables: {
                fragment: `"${ query }"`,
            },
        });

        await router.buildAndCommitLocation({
            pathname: "/search",
            state: {
                query,
            },
            resetScroll: false,
        });
    }

    return (
        <section>
            <SearchContainer className={ style({ backgroundColor: "layer-2", height: "full", padding: "text-to-control", margin: "text-to-control", borderRadius: "sm" }) }>
                <TextField
                    styles={ style({ width: "full" }) }
                    aria-label="Search transcript"
                    type="search"
                    enterKeyHint="search"
                    inputMode="search"
                    prefix={ <SearchIcon /> }
                    size="M"
                    value={ query }
                    onChange={ setQuery }
                    onKeyDown={ event => {
                        if (event.key === "Enter") {
                            runSearchHandler();
                        }
                    } } />
                <Button variant="primary" size="M" isPending={ loading } onPress={ runSearchHandler }>
                    <SearchIcon />
                    <Text>Search</Text>
                </Button>
            </SearchContainer>
            <ResultsContainer className={ style({ backgroundColor: "layer-2", height: "full", padding: "text-to-control", margin: "text-to-control", borderRadius: "sm" }) }>
                <div className={ style({ width: "max", height: "max" }) }>
                    <h3>Search results</h3>
                    { loading && spinner }
                    { error && <p>Error: { error.message }</p> }
                    { data?.search?.statementText?.length === 0 && <p>No results found.</p> }
                    { data?.search?.statementText?.map(result => (
                        <div key={ result.statement.uid } className={ style({ marginBottom: "text-to-control", backgroundColor: "layer-1" }) }>
                            <p><strong>Interview { result.interview.number } - { result.interview.interviewee.name }</strong></p>
                            <p>{ result.statement.text }</p>
                            <p><em>Start time: { result.statement.startTime } | End time: { result.statement.endTime }</em></p>
                            <hr />
                        </div>
                    )) }
                </div>
            </ResultsContainer>
        </section>
    );
}
