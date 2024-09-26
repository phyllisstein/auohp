import {useEffect, useState} from 'react'

import {useNeo4j} from 'hooks/infrastructure'


export interface WithLabels {
  labels: string[]
}

export interface Person extends WithLabels {
  name: string
  uid: string
}

export interface Statement extends WithLabels {
  text: string
}

export interface Interview extends WithLabels {
  number: number
  uid: string
}

export interface Documentary extends WithLabels {
  date: string
  title: string
  uid: string
  slug: string
}

export interface Video extends WithLabels {
  url: string
  uid: string
}

export interface Neo4jResult {
  person: Person
  speakerSays: SaysEdge
  statement: Statement
  video: Video
  artefact: Documentary | Interview
}

interface SaysEdge {
  startTimestamp: string
  endTimestamp: string
  startTime: number
  endTime: number
}

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
                    MATCH (statement)<-[speakerSays:SAYS]-(speaker)
                    MATCH (speaker)<-[:INCLUDES_SPEAKER]-()<-[:HAS_TRANSCRIPT]-(video)<-[:HAS_VIDEO]-(artefact)
                    OPTIONAL MATCH (speaker) <-[:INTERVIEWED_AS]- (person)
                    RETURN statement, person, speakerSays, artefact, video
                    ORDER BY score DESC
                `, {index, query})

      const searchResults = result.records.map(record => {
        return {
          person: record.get('person'),
          speakerSays: record.get('speakerSays'),
          statement: record.get('statement'),
          artefact: record.get('artefact'),
          video: record.get('video'),
        }
      })

      setSearchResults(searchResults)
    }

    void search()
  }, [query, index, driver])

  return searchResults
}
