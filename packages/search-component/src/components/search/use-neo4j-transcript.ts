import { useEffect, useState } from 'react'

import { useNeo4j } from './use-neo4j'

export interface Neo4jResult {
    uid: string
    startTime: number
    endTime: number
    speaker: string
    statement: string
    score: number
    interviewURL: string
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
                    CALL db.index.fulltext.queryNodes($index, $query) YIELD node, score
                    MATCH (node)<-[sez:SAYS]-(speaker:Speaker)<-[:INTERVIEWED_AS]-(whom)
                    MATCH (speaker)<-[:INTERVIEWED_WITH]-(interview)
                    RETURN sez.startTime AS startTime,
                        sez.endTime AS endTime,
                        sez.duration AS duration,
                        whom.name AS speaker,
                        node.text AS statement,
                        node.uid AS uid,
                        interview.url AS interviewURL,
                        score
                `, { index, query })

            const searchResults = result.records.map(record => ({
                startTime: record.get('startTime'),
                endTime: record.get('endTime'),
                speaker: record.get('speaker'),
                statement: record.get('statement'),
                score: record.get('score'),
                interviewURL: record.get('interviewURL'),
                uid: record.get('uid'),
            }))

            setSearchResults(searchResults)
        }

        void search()
    }, [query, index, driver])

    return searchResults
}
