/**
 * Parse a JSON transcription produced by AWS Transcribe, assembling complete
 * spoken segments from the individual words.
 */

import * as fs from 'fs/promises'
import path from 'path'
import { fileURLToPath } from 'url'

import { nanoid } from 'nanoid'
import neo4j, { EagerResult } from 'neo4j-driver'

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
    {
        disableLosslessIntegers: true,
    },
)

async function seed({ data, date, interviewee, interviewNumber }: any) {
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

        const startTime = Number.parseFloat(segment.start_time)
        const endTime = Number.parseFloat(segment.end_time)
        const duration = endTime - startTime

        return {
            startTime,
            endTime,
            duration,
            speaker: segment.speaker_label,
            text: text.trim(),
            statementUID: nanoid(),
        }
    })

    await neo4jDriver.executeQuery(`
        MATCH (jim:Person {name: 'Jim Hubbard'})
        MATCH (sarah:Person {name: 'Sarah Schulman'})
        WITH jim, sarah
        CREATE (i:Interview {number: $interviewNumber, date: date($date), url: $url, uid: $interviewUID})
        CREATE (interviewee:Person {name: $interviewee, uid: $intervieweeUID})
        CREATE (sarahSpeaker:Speaker {remoteID: 'spk_2'})
        CREATE (jimSpeaker:Speaker {remoteID: 'spk_1'})
        CREATE (intervieweeSpeaker:Speaker {remoteID: 'spk_0'})
        MERGE (interviewee)-[:INTERVIEWED_AS]->(intervieweeSpeaker)
        MERGE (jim)-[:INTERVIEWED_AS]->(jimSpeaker)
        MERGE (sarah)-[:INTERVIEWED_AS]->(sarahSpeaker)
        CREATE (i)-[:INTERVIEWED_WITH]->(intervieweeSpeaker)
        CREATE (i)-[:INTERVIEWED_WITH]->(jimSpeaker)
        CREATE (i)-[:INTERVIEWED_WITH]->(sarahSpeaker)`,
    {
        date,
        interviewee,
        intervieweeUID: nanoid(),
        interviewUID: nanoid(),
        interviewNumber,
        url: 'http://localhost:4000/index.html',
    },
    )

    for await (let segment of segments) {
        const params = {
            ...segment,
            interviewNumber,
        }

        const result: EagerResult = await neo4jDriver.executeQuery(`
            MATCH (speaker:Speaker {remoteID: $speaker}) <-[:INTERVIEWED_WITH]- (interview:Interview {number: $interviewNumber})
            MATCH (speaker) <-[:INTERVIEWED_AS]- (person:Person)
            CREATE (statement:Statement {text: $text, uid: $statementUID})
            MERGE (speaker)-[:SAYS {startTime: $startTime, endTime: $endTime, duration: $duration}]->(statement)
            RETURN statement, person, interview
        `, params)

        if (!result?.records?.length) {
            console.error(`
                No records returned for segment:Could not create node for
                segment starting ${ segment.startTime } in interview
                ${ interviewNumber } for speaker ${ segment.speaker }.\n\n\n
                ${ JSON.stringify(result, null, 2) }
            `)
        }
    }
}

async function bootstrap() {
    await neo4jDriver.getServerInfo()

    await neo4jDriver.executeQuery(
        // language=Cypher
        `
          MATCH p = ()--()
          DETACH DELETE p
        `,
    )

    await neo4jDriver.executeQuery(
    // language=Cypher
        `
          DROP INDEX transcript_search IF EXISTS
        `,
    )

    await neo4jDriver.executeQuery(
    // language=Cypher
        `
          DROP INDEX name_search IF EXISTS
        `,
    )

    for await (let label of NEO4J_LABELS) {
        await neo4jDriver.executeQuery(`
            CREATE CONSTRAINT ${ label }UID IF NOT EXISTS
            FOR (n:${ label }) REQUIRE n.uid IS UNIQUE
            `,
        { label },
        )
    }

    await neo4jDriver.executeQuery(`
        CREATE FULLTEXT INDEX transcript_search IF NOT EXISTS
        FOR (n:Statement) ON EACH [n.text]
        OPTIONS {
            indexConfig: {
                \`fulltext.eventually_consistent\`: true
            }
        }
    `)

    await neo4jDriver.executeQuery(`
        CREATE (jim:Person {name: 'Jim Hubbard', uid: $jimUID})
        CREATE (sarah:Person {name: 'Sarah Schulman', uid: $sarahUID})
    `, { jimUID: nanoid(), sarahUID: nanoid() })
}

async function main() {
    await bootstrap()

    let data: AWSTranscribeResult

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
