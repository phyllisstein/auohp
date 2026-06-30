import queryString from "query-string";
import { useEffect, useRef, useState } from "react";
import { SearchContainer, SearchResult, SearchResults, ResultMatch, ResultSource, ResultTimestamp, SearchInput } from "./search-styles";
import { Results } from "./results";
import deepEqual from "fast-deep-equal";
import { gql } from "@apollo/client";
import { useLazyQuery } from "@apollo/client/react";


const SEARCH_QUERY = gql`
    query SearchStatements($search: String!) {
        searchStatements(
            query: $search
            limit: 15
        ) {
            interview {
                number
            }
            statement {
                text
                startTime
                endTime
                person {
                    name
                }
            }
        }
    }
`;


const formatTimestamp = (timestamp: number) =>
    Temporal.Duration.from({ seconds: Math.round(timestamp) })
        .round({
            largestUnit: "hours",
            smallestUnit: "seconds",
        })
        .toLocaleString("en-US", {
            style: "digital",
            hoursDisplay: "auto",
            hours: "numeric",
        });


export function Search() {
    const searchBox = useRef<HTMLInputElement>(null);
    const [boxRect, setBoxRect] = useState<DOMRect | null>(null);
    const [executeSearch, searchQuery] = useLazyQuery(SEARCH_QUERY, {
        fetchPolicy: "network-only",
    });

    const handleResultClick = result => {
        const url = `/${ result.interview.number }`;

        const nextURL = queryString.stringifyUrl({
            query: {
                timestamp: result.statement.startTime,
            },
            url: url,
        });

        window.location.href = nextURL;
    };

    useEffect(() => {
        if (searchBox.current) {
            const nextBox = searchBox.current.getBoundingClientRect().toJSON();
            if (!deepEqual(nextBox, boxRect)) {
                setBoxRect(nextBox);
            }
        }
    }, []);

    return (
        <SearchContainer className="search-container">
            <SearchInput
                ref={ searchBox }
                type="search"
                // FIXME: Should be a deferred value or transition
                onChange={ e => executeSearch({ variables: { search: e.target.value } }) } />
            <Results { ...boxRect }>
                <SearchResults>
                    {
                        searchQuery.data?.searchStatements.map(hit => (
                            <SearchResult key={ `${ hit.statement.startTime }-${ hit.statement.uid }` } onClick={ () => handleResultClick(hit) }>
                                <ResultMatch>{ hit.statement.text }</ResultMatch>
                                <ResultSource>{ hit.statement.person.name }</ResultSource>
                                <ResultTimestamp>{ formatTimestamp(hit.statement.startTime) }</ResultTimestamp>
                            </SearchResult>
                        ))
                    }
                </SearchResults>
            </Results>
        </SearchContainer>
    );
}
