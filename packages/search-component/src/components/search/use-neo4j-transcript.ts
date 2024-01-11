import { useEffect, useState } from 'react'

import { useNeo4j } from './use-neo4j'

export interface Neo4jResult {
    uuid: number
    startTime: number
    endTime: number
    speaker: string
    statement: string
    score: number
    interviewURL: string
}

export function useNeo4jTranscript(query: string, index: string): Neo4jResult[] {
    const driver = useNeo4j('bolt://localhost:7687', 'neo4j', 'auohpauohp')
    // const driver = useNeo4j('bolt://bolt.auohp.here:7687', 'neo4j', 'auohpauohp')
    const [searchResults, setSearchResults] = useState<Neo4jResult[]>([])

    useEffect(() => {
        if (!driver || !index || !query) {
            setSearchResults([])
            return
        }

        async function search() {
            const result = await driver.executeQuery(`
                CALL db.index.fulltext.queryNodes($index, $query) YIELD node, score
                MATCH (node)<-[sez:SAYS]-(speaker:Speaker)<-[:INTERVIEWED_AS]-(whom)
                MATCH (speaker)<-[:INTERVIEWED_WITH]-(interview)
                RETURN sez.startTime AS startTime, sez.endTime AS endTime,
                whom.name AS speaker,
                node.text AS statement,
                interview.url AS interviewURL,
                node.uuid AS uuid,
                score
            `, { index, query })

            const searchResults = result.records.map(record => ({
                startTime: record.get('startTime'),
                endTime: record.get('endTime'),
                speaker: record.get('speaker'),
                statement: record.get('statement'),
                score: record.get('score'),
                interviewURL: record.get('interviewURL'),
                uuid: record.get('uuid'),
            }))

            setSearchResults(searchResults)
        }

        void search()
    }, [query, index, driver])

    return searchResults
}
