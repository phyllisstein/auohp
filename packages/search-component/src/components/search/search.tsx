import queryString from 'query-string'
import { useState, useTransition } from 'react'

import { type SearchResult, useNeo4jSearch } from './use-neo4j-search'
import './search.scss'

export function Search () {
    const [search, setSearch] = useState<string>('')
    const [searchTransitioning, searchTransition] = useTransition()

    const handleSearch = (e: React.ChangeEvent<HTMLInputElement>) => {
        searchTransition(() => {
            setSearch(e.target.value)
        })
    }

    const handleResultClick = (result: SearchResult) => {
        const hash = queryString.stringify({
            ...queryString.parse(location.hash),
            timestamp: result.startTime,
        })

        window.location.hash = hash
    }

    const searchResults = useNeo4jSearch(search, 'transcriptSearch')

    return (
        <div className='search-container'>
            <input type='search' onChange={ handleSearch } />
            <div>
                {
                    !searchTransitioning && searchResults.map(result => (
                        <div key={ result.uuid } onClick={ () => handleResultClick(result) }>
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
