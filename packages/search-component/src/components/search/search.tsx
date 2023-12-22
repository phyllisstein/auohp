import { useState, useTransition } from 'react'

import { useNeo4jSearch } from './use-neo4j-search'

export function Search () {
    const [search, setSearch] = useState<string>('')
    const [results, setResults] = useState<string[]>([])
    const [_, startTransition] = useTransition()

    const handleSearch = (e: React.ChangeEvent<HTMLInputElement>) => {
        startTransition(() => {
            setSearch(e.target.value)
        })
    }

    const searchResults = useNeo4jSearch(search, 'transcriptSearch')

    return (
        <>
            <input type='search' onChange={ handleSearch } />
            <div>
                {
                    searchResults.map(result => (
                        <div key={ result.id }>
                            <h3>{ result.statement }</h3>
                            <p>{ result.speaker }</p>
                        </div>
                    ))
                }
            </div>
        </>
    )
}
