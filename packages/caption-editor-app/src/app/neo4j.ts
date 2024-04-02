'use server'

import neo4j, { type Driver } from 'neo4j-driver'

import searchSpeakerCaptions from './search-speaker-captions.cypher'

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

const NEO4J_URI = process?.env?.NEO4J_URI || 'neo4j://localhost:7687'
const NEO4J_USER = process?.env?.NEO4J_USER || 'neo4j'
const NEO4J_PASSWORD = process?.env?.NEO4J_PASSWORD || 'neo4j'

let driver: Driver | null = null
export async function connect(
    uri: string = NEO4J_URI,
    username: string = NEO4J_USER,
    password: string = NEO4J_PASSWORD,
): Promise<Driver> {
    if (driver) {
        return driver
    }

    driver = neo4j.driver(
        uri,
        neo4j.auth.basic(username, password),
        {
            disableLosslessIntegers: true,
        },
    )

    try {
        const serverInfo =  await driver.getServerInfo()
        console.log(`Connected to ${ serverInfo.address }`)
    } catch (err) {
        console.error('Failed to connect to Neo4j')
        console.error(err)
        await driver.close()
        throw new Error('Failed to connect to Neo4j')
    }

    return driver
}

export async function disconnect() {
    if (!driver) {
        return
    }

    await driver.close()
    console.log('Disconnected from Neo4j')
}

export async function transcriptSearch(query: string, index: string = 'transcript_search'): Promise<Neo4jResult[]> {
    if (!query || !index) {
        return []
    }

    driver = await connect()
    const result = await driver.executeQuery(
        searchSpeakerCaptions,
        { index, query },
    )

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

    return searchResults
}
