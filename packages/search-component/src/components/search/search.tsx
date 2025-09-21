import { type Neo4jResult, isDocumentary, isInterview, isBroadsheet, useNeo4jTranscript } from "hooks/interviews";
import { debounce } from "lodash-es";
import queryString from "query-string";
import { type ChangeEvent, useEffect, useRef, useState, useTransition } from "react";
import { SearchContainer, SearchInput, SearchResult, SearchResults, ResultImage, ResultMatch, ResultSource, ResultTimestamp } from "./search-styles";
import { Results } from "./results";
import deepEqual from "fast-deep-equal";


export function Search() {
    const [search, setSearch] = useState<string>("");
    const [searchTransitioning, searchTransition] = useTransition();
    const searchBox = useRef<HTMLInputElement>(null);
    const [boxRect, setBoxRect] = useState<DOMRect | null>(null);

    const handleSearch = debounce((e: ChangeEvent<HTMLInputElement>) => {
        searchTransition(() => {
            setSearch(e.target.value);
        });
    }, 500);

    const handleResultClick = (result: Neo4jResult) => {
        const url = isDocumentary(result.artefact)
            ? `/${ result.artefact.properties.slug }`
            : isInterview(result.artefact)
                ? `/${ result.artefact.properties.number }`
                : "";

        if (!url) return;

        const nextURL = queryString.stringifyUrl({
            query: {
                timestamp: result.meta.properties.startTime,
            },
            url,
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
    });

    const neo4jResults = useNeo4jTranscript(search);

    return (
        <SearchContainer className="search-container">
            <SearchInput ref={ searchBox } type="search" onChange={ handleSearch } />
            <Results { ...boxRect }>
                <SearchResults>
                    {
                        !searchTransitioning && neo4jResults.map(result => (
                            <SearchResult key={ `${ result.meta.properties.startTime }-${ result.artefact.properties.uid }` } onClick={ () => handleResultClick(result) }>
                                {
                                    isBroadsheet(result.artefact)
                                        ? <ResultImage src={ result.asset.properties.url } />
                                        : <ResultMatch>{ result.statement.properties.text }</ResultMatch>
                                }
                                <ResultSource>
                                    <strong>
                                        {
                                            isDocumentary(result.artefact)
                                                ? result.artefact.properties.title
                                                : isInterview(result.artefact)
                                                    ? result.person.properties.name
                                                    : isBroadsheet(result.artefact)
                                                        ? result.artefact.properties.title
                                                        : ""
                                        }
                                    </strong>
                                </ResultSource>
                                <ResultTimestamp>{ result.meta.properties.startTimestamp }</ResultTimestamp>
                            </SearchResult>
                        ))
                    }
                </SearchResults>
            </Results>
        </SearchContainer>
    );
}
