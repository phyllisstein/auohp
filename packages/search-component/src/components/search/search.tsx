import queryString from 'query-string'
import { type ChangeEvent, useState, useTransition } from 'react'

import { type Neo4jResult, useNeo4jTranscript } from 'hooks/interviews'

import './search.scss'

export function Search() {
    const [search, setSearch] = useState<string>('')
    const [searchTransitioning, searchTransition] = useTransition()

    const handleSearch = (e: ChangeEvent<HTMLInputElement>) => {
        searchTransition(() => {
            setSearch(e.target.value)
        })
    }

    const handleResultClick = (result: Neo4jResult) => {
        const nextURL = queryString.stringifyUrl({
            query: {
                timestamp: result.timestamp,
            },
            url: `/${ result.interviewNumber }`,
        })

        window.location.href = nextURL
    }

    const neo4jResults = useNeo4jTranscript(search, 'transcript_search')

    return (
        <div className='search-container'>
            <input type='search' onChange={ handleSearch } />
            <div>
                {
                    !searchTransitioning && neo4jResults.map(result => (
                        <div key={ `${ result.timestamp }-${ result.interviewNumber }` } onClick={ () => handleResultClick(result) }>
                            <h3>{ result.statement }</h3>
                            <p>{ result.speaker }</p>
                            <aside>{ result.timestamp }</aside>
                        </div>
                    ))
                }
            </div>
        </div>
    )
}
