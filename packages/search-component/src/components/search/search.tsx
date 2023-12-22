import queryString from 'query-string'
import { useState, useTransition } from 'react'

import { SearchResult, useNeo4jSearch } from './use-neo4j-search'

export function Search () {
    const [search, setSearch] = useState<string>('')
    const [_, startTransition] = useTransition()

    const handleSearch = (e: React.ChangeEvent<HTMLInputElement>) => {
        startTransition(() => {
            setSearch(e.target.value)
        })
    }

    const handleResultClick = (result: SearchResult) => {
        // window.history.pushState(null, '', queryString.stringifyUrl({
        //     url: window.location.href,
        //     query: {
        //         ...queryString.parse(location.search),
        //         timestamp: result.startTime,
        //     },
        // }))
        const hash = queryString.stringify({
            ...queryString.parse(location.hash),
            timestamp: result.startTime,
        })

        window.location.hash = hash
    }

    const searchResults = useNeo4jSearch(search, 'transcriptSearch')

    return (
        <>
            <input type='search' onChange={ handleSearch } />
            <div>
                {
                    searchResults.map(result => (
                        <div key={ result.uuid } onClick={ () => handleResultClick(result) }>
                            <h3>{ result.statement }</h3>
                            <p>{ result.speaker }</p>
                            <aside>{ result.startTime }</aside>
                        </div>
                    ))
                }
            </div>
        </>
    )
}
