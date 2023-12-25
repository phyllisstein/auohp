/**
 * Parse a JSON transcription produced by AWS Transcribe, assembling complete
 * spoken segments from the individual words.
 */

import * as fs from 'fs/promises'
import path from 'path'
import { fileURLToPath } from 'url'

import { Client } from '@elastic/elasticsearch'
import neo4j from 'neo4j-driver'

const NEO4J_LABELS = [
    'Interview',
    'Person',
    'Speaker',
    'Statement',
]

const __filename = fileURLToPath(import.meta.url)
const __dirname = path.dirname(__filename)

const neo4jDriver = neo4j.driver(
    'bolt://localhost:7687',
    neo4j.auth.basic('neo4j', 'auohpauohp'),
)

const esClient = new Client({
    node: 'https://elastic.auohp.here',
})

async function seed ({ data, date, interviewee, interviewNumber }: any) {
    if (
        !Array.isArray(data?.results?.speaker_labels?.segments) ||
        data.results.speaker_labels.segments.length === 0
    ) {
        throw new Error('No speaker labels found')
    }

    const segments = data.results.speaker_labels.segments.map(segment => {
        const text = segment.items.reduce((acc, item) => {
            const word = data.results.items.find(
                i => i.start_time === item.start_time,
            )?.alternatives[0].content
            return `${ acc } ${ word }`
        }, '')

        return {
            startTime: Number.parseFloat(segment.start_time),
            endTime: Number.parseFloat(segment.end_time),
            speaker: segment.speaker_label,
            text: text.trim(),
        }
    })

    await neo4jDriver.executeQuery(`
        CREATE (i:Interview {number: $interviewNumber, date: date($date), url: $url})
        MERGE (interviewee:Person {name: $interviewee})
        MERGE (jim:Person {name: 'Jim Hubbard'})
        MERGE (sarah:Person {name: 'Sarah Schulman'})
        CREATE (sarahSpeaker:Speaker {remoteID: 'spk_2'})
        CREATE (jimSpeaker:Speaker {remoteID: 'spk_1'})
        CREATE (intervieweeSpeaker:Speaker {remoteID: 'spk_1'})
        MERGE (interviewee)-[:INTERVIEWED_AS]->(intervieweeSpeaker)
        MERGE (jim)-[:INTERVIEWED_AS]->(jimSpeaker)
        MERGE (sarah)-[:INTERVIEWED_AS]->(sarahSpeaker)
        CREATE (i)-[:INTERVIEWED_WITH]->(intervieweeSpeaker)
        CREATE (i)-[:INTERVIEWED_WITH]->(jimSpeaker)
        CREATE (i)-[:INTERVIEWED_WITH]->(sarahSpeaker)
    `, {
        date,
        interviewee,
        interviewNumber,
        url: 'http://localhost:4000/index.html',
    })

    for await (let segment of segments) {
        const { records } = await neo4jDriver.executeQuery(`
            MATCH (speaker:Speaker {remoteID: $speakerID}) <-[:INTERVIEWED_WITH]- (interview:Interview {number: $interviewNumber})
            MATCH (speaker) <-[:INTERVIEWED_AS]- (person:Person)
            CREATE (statement:Statement {text: $text})
            SET statement.uuid = randomUUID()
            MERGE (speaker)-[:SAYS {startTime: $startTime, endTime: $endTime}]->(statement)
            RETURN statement.uuid AS statementID, person.uuid AS personID, interview.uuid AS interviewID
        `, {
            text: segment.text,
            speakerID: segment.speaker,
            startTime: segment.startTime,
            endTime: segment.endTime,
            interviewNumber,
        })

        if (records.length === 0) continue

        const statementID = records[0].get('statementID')
        const personID = records[0].get('personID')
        const interviewID = records[0].get('interviewID')

        await esClient.index({
            index: 'transcripts',
            id: statementID,
            document: {
                interview: interviewID,
                person: personID,
                statement: statementID,
                text: segment.text,
                timestamp: {
                    gte: segment.startTime,
                    lte: segment.endTime,
                },
            },
        })
    }
}

async function bootstrap () {
    await neo4jDriver.getServerInfo()

    await neo4jDriver.executeQuery(`
            MATCH p=()--()
            DETACH DELETE p
        `)

    for await (let label of NEO4J_LABELS) {
        await neo4jDriver.executeQuery(
            `
                CALL apoc.uuid.setup($label, 'neo4j')
            `,
            { label },
            { database: 'system' })

        await neo4jDriver.executeQuery(`
            CREATE CONSTRAINT ${ label }ID IF NOT EXISTS
            FOR (n:${ label }) REQUIRE n.uuid IS UNIQUE
        `, { label })
    }

    await neo4jDriver.executeQuery(`
        CREATE FULLTEXT INDEX transcriptSearch IF NOT EXISTS
        FOR (n:Statement) ON EACH [n.text]
        OPTIONS {
            indexConfig: {
                \`fulltext.analyzer\`: 'english',
                \`fulltext.eventually_consistent\`: true
            }
        }
    `)

    try {
        await esClient.indices.delete({ index: 'transcripts' })
    } catch (_) {}

    await esClient.indices.create({
        index: 'transcripts',
        body: {
            mappings: {
                properties: {
                    interview: { type: 'keyword' },
                    person: { type: 'keyword' },
                    statement: { type: 'keyword' },
                    text: { type: 'text' },
                    timestamp: {
                        type: 'float_range',
                    },
                },
            },
        },
    })
}

async function main () {
    await bootstrap()

    let data

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~ 004 - Gregg Bordowitz ~~~~~~~~~~~~~~~~~~~~~~~~~ //
    data = JSON.parse(
        await fs.readFile(
            path.join(__dirname, '../assets/004_gregg_bordowitz.json'),
            'utf8',
        ),
    )
    await seed({
        data,
        date: '2020-05-27',
        interviewee: 'Gregg Bordowitz',
        interviewNumber: 4,
    })
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~ //

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~ 012 - Mark Harrington ~~~~~~~~~~~~~~~~~~~~~~~~~ //
    data = JSON.parse(
        await fs.readFile(
            path.join(__dirname, '../assets/012_mark_harrington.json'),
            'utf8',
        ),
    )
    await seed({
        data,
        date: '2020-05-27',
        interviewee: 'Mark Harrington',
        interviewNumber: 12,
    })
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~ //

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~ 035 - Larry Kramer ~~~~~~~~~~~~~~~~~~~~~~~~~~~ //
    data = JSON.parse(
        await fs.readFile(
            path.join(__dirname, '../assets/035_larry_kramer.json'),
            'utf8',
        ),
    )
    await seed({
        data,
        date: '2020-05-27',
        interviewee: 'Larry Kramer',
        interviewNumber: 35,
    })
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~ //

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~ 074 - Douglas Crimp ~~~~~~~~~~~~~~~~~~~~~~~~~~ //
    data = JSON.parse(
        await fs.readFile(
            path.join(__dirname, '../assets/074_douglas_crimp.json'),
            'utf8',
        ),
    )
    await seed({
        data,
        date: '2020-05-27',
        interviewee: 'Douglas Crimp',
        interviewNumber: 74,
    })
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~ //

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~ 138 - Joan Gibbs ~~~~~~~~~~~~~~~~~~~~~~~~~~~~ //
    data = JSON.parse(
        await fs.readFile(
            path.join(__dirname, '../assets/138_joan_gibbs.json'),
            'utf8',
        ),
    )
    await seed({
        data,
        date: '2020-05-27',
        interviewee: 'Joan Gibbs',
        interviewNumber: 138,
    })
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~ //

    await neo4jDriver.close()
}

void (await main())
