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
                timestamp: result.speakerSays.startTime,
            },
            url: `/${ result.interview.number }`,
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
                        <div key={ `${ result.speakerSays.startTime }-${ result.interview.uid }` } onClick={ () => handleResultClick(result) }>
                            <h3>{ result.statement.text }</h3>
                            <p>{ result.person.name }</p>
                            <aside>{ result.speakerSays.startTimestamp }</aside>
                        </div>
                    ))
                }
            </div>
        </div>
    )
}
