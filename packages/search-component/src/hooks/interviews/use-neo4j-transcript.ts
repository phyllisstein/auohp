import {useEffect, useState} from 'react'

import {useNeo4j} from 'hooks/infrastructure'

export interface Person {
  name: string
  uid: string
}

export interface Statement {
  text: string
}

export interface Interview {
  number: number
  uid: string
}

export interface Video {
  url: string
}

export interface Neo4jResult {
  person: Person
  speakerSays: SaysEdge
  statement: Statement
  interview: Interview
  video: Video
}

interface SaysEdge {
  startTimestamp: string
  endTimestamp: string
  startTime: number
  endTime: number
}

export function useNeo4jTranscript(query: string, index: string): Neo4jResult[] {
  const driver = useNeo4j('bolt+s://bolt.auohp.here:443', 'neo4j', 'auohpauohp')
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
                    MATCH (statement)<-[speakerSays:SAYS]-(speaker:Interviewee)<-[:INTERVIEWED_AS]-(person)
                    MATCH (speaker)<-[:INCLUDES_SPEAKER]-(transcript)<-[:HAS_TRANSCRIPT]-(video)<-[:HAS_VIDEO]-(interview)
                    RETURN statement, person, speakerSays, interview, video
                    ORDER BY score DESC
                `, {index, query})

      const searchResults = result.records.map(record => ({
        person: record.get('person').properties,
        speakerSays: record.get('speakerSays').properties,
        statement: record.get('statement').properties,
        interview: record.get('interview').properties,
        video: record.get('video').properties,
      }))

      setSearchResults(searchResults)
    }

    void search()
  }, [query, index, driver])

  return searchResults
}
