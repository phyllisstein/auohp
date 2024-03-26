import { useEffect, useState } from 'react'

import { useNeo4j } from 'hooks/infrastructure'

export interface Neo4jResult {
    endTime: number
    interviewNumber: number
    score: number
    speakerName: string
    startTime: number
    statement: string
    statementUID: string
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
                    CALL db.index.fulltext.queryNodes($index, $query) YIELD node, score
                    MATCH (node)<-[speakerSays:SAYS]-(speaker:Speaker)<-[:INTERVIEWED_AS]-(person:Person)
                    MATCH (speaker)<-[:INTERVIEWED_WITH]-(interview)-[:HAS_VIDEO]->(video)
                    RETURN speakerSays.startTime AS startTime,
                        speakerSays.endTime AS endTime,
                        speakerSays.duration AS duration,
                        person.name AS speakerName,
                        node.text AS statement,
                        node.uid AS statementUID,
                        interview.number AS interviewNumber,
                        video.url AS videoURL,
                        score
                    ORDER BY score DESC
                `, { index, query })

            const searchResults = result.records.map(record => ({
                endTime: record.get('endTime'),
                interviewNumber: record.get('interviewNumber'),
                score: record.get('score'),
                speakerName: record.get('speakerName'),
                startTime: record.get('startTime'),
                statement: record.get('statement'),
                statementUID: record.get('statementUID'),
                videoURL: record.get('videoURL'),
            }))

            setSearchResults(searchResults)
        }

        void search()
    }, [query, index, driver])

    return searchResults
}
