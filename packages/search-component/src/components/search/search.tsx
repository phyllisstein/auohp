import queryString from 'query-string'
import {type ChangeEvent, useState, useTransition} from 'react'
import {debounce} from 'lodash-es'

import {type Neo4jResult, type Documentary, type Interview, useNeo4jTranscript} from 'hooks/interviews'

import './search.scss'

function isDocumentary(artefact: Documentary | Interview): artefact is Documentary {
  return Array.isArray(artefact.labels) && artefact.labels.includes('Documentary')
}

export function Search() {
  const [search, setSearch] = useState<string>('')
  const [searchTransitioning, searchTransition] = useTransition()
  const debouncedSearch = debounce(setSearch, 500)

  const handleSearch = (e: ChangeEvent<HTMLInputElement>) => {
    debouncedSearch(e.target.value)
  }

  const handleResultClick = (result: Neo4jResult) => {
    const url = isDocumentary(result.artefact)
      ? `/${result.artefact.properties.slug}`
      : `/${result.artefact.properties.number}`
    const nextURL = queryString.stringifyUrl({
      query: {
        timestamp: result.speakerSays.properties.startTime,
      },
      url,
    })

    window.location.href = nextURL
  }

  const neo4jResults = useNeo4jTranscript(search, 'transcript_search')

  return (
    <div className='search-container'>
      <input type='search' onChange={handleSearch} />
      <div>
        {
          !searchTransitioning && neo4jResults.map(result => (
            <div key={`${result.speakerSays.properties.startTime}-${result.artefact.properties.uid}`} onClick={() => handleResultClick(result)}>
              <h3>{result.statement.properties.text}</h3>
              <p>
                {
                  isDocumentary(result.artefact)
                    ? result.artefact.properties.title
                    : result.person.properties.name
                }
              </p>
              <aside>{result.speakerSays.properties.startTimestamp}</aside>
            </div>
          ))
        }
      </div>
    </div>
  )
}
