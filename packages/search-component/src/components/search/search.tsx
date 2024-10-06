import {type Neo4jResult, isDocumentary, isInterview, isLeaflet, useNeo4jTranscript} from 'hooks/interviews'
import {debounce} from 'lodash-es'
import queryString from 'query-string'
import {type ChangeEvent, useState, useTransition} from 'react'
import {SearchContainer, SearchInput, SearchResult, SearchResults} from './search-styles'


export function Search() {
  const [search, setSearch] = useState<string>('')
  const [searchTransitioning, searchTransition] = useTransition()

  const handleSearch = debounce((e: ChangeEvent<HTMLInputElement>) => {
    searchTransition(() => {
      setSearch(e.target.value)
    })
  }, 500)

  const handleResultClick = (result: Neo4jResult) => {
    const url = isDocumentary(result.artefact)
      ? `/${result.artefact.properties.slug}`
      : isInterview(result.artefact)
        ? `/${result.artefact.properties.number}`
        : isLeaflet(result.artefact)
          ? `/${result.artefact.properties.title}`
          : ''
    const nextURL = queryString.stringifyUrl({
      query: {
        timestamp: result.meta.properties.startTime,
      },
      url,
    })

    window.location.href = nextURL
  }

  const neo4jResults = useNeo4jTranscript(search, 'transcript_search')

  return (
    <SearchContainer className='search-container'>
      <SearchInput type='search' onChange={handleSearch} />
      <SearchResults>
        {
          !searchTransitioning && neo4jResults.map(result => (
            <SearchResult key={`${result.meta.properties.startTime}-${result.artefact.properties.uid}`} onClick={() => handleResultClick(result)}>
              <div>
                {result.statement.properties.text}
              </div>
              <div>
                <strong>
                  {
                    isDocumentary(result.artefact)
                      ? result.artefact.properties.title
                      : isInterview(result.artefact)
                        ? result.person.properties.name
                        : isLeaflet(result.artefact)
                          ? result.artefact.properties.title
                          : ''
                  }
                </strong>
              </div>
              <aside>{result.meta.properties.startTimestamp}</aside>
            </SearchResult>
          ))
        }
      </SearchResults>
    </SearchContainer>
  )
}
