import type { EagerResult } from 'neo4j-driver'
import { useEffect, useState } from 'react'

import { useNeo4j } from 'hooks/infrastructure'


interface StatementTranscriptionMeta {
    startTime: number
    endTime: number
    startTimestamp: string
    endTimestamp: string
}

interface Transcript {
    uid: string
}

interface Video {
    uid: string
    url: string
}

interface Person {
    uid: string
    name: string
}

interface DocumentaryArtefact {
    uid: string
    title: string
    date: Date
    labels: ['Documentary']
}

interface InterviewArtefact {
    uid: string
    title: string
    date: Date
    name: string
    number: number
    labels: ['Interview']
}

interface Statement {
    text: string
}

interface Neo4jResult {
    statementTranscriptionMeta: StatementTranscriptionMeta
    transcript: Transcript
    video: Video
    person: Person
    score: number
    artefact: DocumentaryArtefact | InterviewArtefact
    statement: Statement
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
            const result = await driver.executeQuery<EagerResult<Neo4jResult>>(
                // language=Cypher
                `
                    CALL db.index.fulltext.queryNodes($index, $query) YIELD node AS statement, score
                    MATCH (statement) <-[statementTranscriptionMeta:TRANSCRIBED_STATEMENT]-(transcript)
                    MATCH (transcript) -[:TRANSCRIBES]-> (video) <-[:HAS_VIDEO]- (artefact)
                    OPTIONAL MATCH (video) -[:WITH_SPEAKER]-> () <-[:INTERVIEWED_AS]- (person)
                    RETURN statementTranscriptionMeta, transcript, video, person, score, artefact, statement
                    ORDER BY score DESC
                    LIMIT 10
                `, { index, query })

            const searchResults = result.records.map(record => ({
                statementTranscriptionMeta: record.get('statementTranscriptionMeta')?.properties,
                transcript: record.get('transcript')?.properties,
                video: record.get('video')?.properties,
                person: record.get('person')?.properties,
                score: record.get('score')?.properties,
                artefact: record.get('artefact')?.properties,
                statement: record.get('statement')?.properties,
            }))

            setSearchResults(searchResults)
        }

        void search()
    }, [query, index, driver])

    return searchResults
}
