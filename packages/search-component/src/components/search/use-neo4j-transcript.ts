import { useEffect, useState } from 'react'

import { useNeo4j } from './use-neo4j'

export interface Neo4jResult {
    endTime: number
    interviewURL: string
    score: number
    speaker: string
    startTime: number
    statement: string
    uid: string
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
                endTime: record.get('endTime'),
                interviewURL: record.get('interviewURL'),
                score: record.get('score'),
                speaker: record.get('speaker'),
                startTime: record.get('startTime'),
                statement: record.get('statement'),
                uid: record.get('uid'),
            }))

            setSearchResults(searchResults)
        }

        void search()
    }, [query, index, driver])

    return searchResults
}
