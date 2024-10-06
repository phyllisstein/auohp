import {useEffect, useState} from 'react'
import {useNeo4j} from 'hooks/infrastructure'
import type {Neo4jResult} from './types'

export function useNeo4jTranscript(query: string, index: string): Neo4jResult[] {
  const driver = useNeo4j()
  const [searchResults, setSearchResults] = useState<Neo4jResult[]>([])

  useEffect(() => {
    if (!driver || !index || !query) {
      setSearchResults([])
      return
    }

    async function search() {
      const result = await driver.executeQuery(
        // language=Cypher
        `
          CALL db.index.fulltext.queryNodes($index, $query) YIELD node AS statement, score
          MATCH (statement)<-[meta:TRANSCRIBES]-(transcript) <-[:HAS_TRANSCRIPT]- (artefact)
          OPTIONAL MATCH (person) -[:INTERVIEWED_AS]-> (speaker) -[:SAYS]-> (statement)
          WHERE speaker:Interviewee
          OPTIONAL MATCH (artefact) -[:HAS_ASSET]-> (asset)
          RETURN statement, meta, person, speaker, asset, artefact, score
          ORDER BY score DESC
        `, {index, query})

      const searchResults = result.records.map(record => {
        return {
          statement: record.get('statement'),
          meta: record.get('meta'),
          person: record.get('person'),
          artefact: record.get('artefact'),
          asset: record.get('asset'),
          score: record.get('score'),
        }
      }).filter(
        result =>
          !result.artefact.labels.includes('Interview')
          || result.person !== null,
      )

      setSearchResults(searchResults)
    }

    void search()
  }, [query, index, driver])

  return searchResults
}
