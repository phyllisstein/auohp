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
        const hash = queryString.stringify({
            ...queryString.parse(location.hash),
            timestamp: result.startTime,
        })

        window.location.hash = hash
    }

    const neo4jResults = useNeo4jTranscript(search, 'transcript_search')

    return (
        <div className='search-container'>
            <input type='search' onChange={ handleSearch } />
            <div>
                {
                    !searchTransitioning && neo4jResults.map(result => (
                        <div key={ result.uid } onClick={ () => handleResultClick(result) }>
                            <h3>{ result.statement }</h3>
                            <p>{ result.speaker }</p>
                            <aside>{ result.startTime }</aside>
                        </div>
                    ))
                }
            </div>
        </div>
    )
}
