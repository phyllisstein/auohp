import { useEffect, useState } from 'react'

import { useNeo4j } from 'hooks/infrastructure'

export interface Neo4jResult {
    interviewNumber: number
    interviewURL: string
    score: number
    speaker: string
    statement: string
    timestamp: number
    videoURL: string
}

export function useNeo4jTranscript(query: string, index: string): Neo4jResult[] {
    const driver = useNeo4j('bolt://localhost:7687', 'neo4j', 'auohpauohp')
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
                    MATCH (speaker)<-[:INTERVIEWED_WITH]-(interview)-[:HAS_VIDEO]->(video)
                    RETURN speakerSays.startTime AS timestamp,
                        person.name AS speaker,
                        statement.text AS statement,
                        interview.number AS interviewNumber,
                        interview.url AS interviewURL,
                        video.url AS videoURL,
                        score
                    ORDER BY score DESC
                `, { index, query })

            const searchResults = result.records.map(record => ({
                interviewNumber: record.get('interviewNumber'),
                score: record.get('score'),
                speaker: record.get('speaker'),
                statement: record.get('statement'),
                timestamp: record.get('timestamp'),
                videoURL: record.get('videoURL'),
            }))

            setSearchResults(searchResults)
        }

        void search()
    }, [query, index, driver])

    return searchResults
}
