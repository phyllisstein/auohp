/**
 * Parse a JSON transcription produced by AWS Transcribe, assembling complete
 * spoken segments from the individual words.
 */

import * as fs from 'fs/promises'
import path from 'path'
import { fileURLToPath } from 'url'

import round from 'lodash.round'
import { nanoid } from 'nanoid'
import neo4j, { type EagerResult, int } from 'neo4j-driver'

const NEO4J_LABELS = [
    'Interview',
    'Person',
    'Speaker',
    'Statement',
    'Video',
]

const __filename = fileURLToPath(import.meta.url)
const __dirname = path.dirname(__filename)

const {
    NEO4J_PASSWORD = 'auohpauohp',
    NEO4J_URI = 'bolt://127.0.0.1:7687',
    NEO4J_USERNAME = 'neo4j',
} = process.env

const neo4jDriver = neo4j.driver(
    NEO4J_URI,
    neo4j.auth.basic(NEO4J_USERNAME, NEO4J_PASSWORD),
    {
        disableLosslessIntegers: true,
    },
)

/**
 * Captions are generated in a two-step process:
 *
 * 1. Store AWS Transcribe results in database as timestamped plain-text hunks,
 *    which the user can correct and edit.
 * 2. Generate static caption assets from hunks, which the user can format and
 *    regenerate.
 *
 * "Source of truth" is the persisted hunks. Corrected and edited hunks are
 * saved in the database. Static captions are generated from these hunks, and
 * regenerated when hunks are updated or caption formatting changes.
 *
 * Hunks are the smallest unit of storage, persisted directly into the database.
 * Edits to a hunk are assumed to occupy the same time frame as the original
 * hunk. (TK: Timestamp editing might be useful, but cumbersome to cascade
 * across many hunks.) This means that once a hunk has been defined, it will not
 * be split or merged. Captions will work along the same assumptions: they
 * follow hunk timestamps and are not split or merged, even if their text
 * becomes too long.
 *
 * Premiere subtitle defaults:
 *
 * - Each subtitle must be less than 42 characters per line
 * - Each subtitle must be no more than 2 lines
 * - Each subtitle must be on screen for at least 3 seconds
 *
 * TODO: BBC has comprehensive guide to best practices:
 *     <https://www.bbc.co.uk/accessibility/forproducts/guides/subtitles/>
 */
async function seed({ data, date, interviewee, interviewNumber, speakers, videoURL }: any) {
    if (
        !Array.isArray(data?.results?.speaker_labels?.segments)
        || data.results.speaker_labels.segments.length === 0
    ) {
        throw new Error('No speaker labels found')
    }

    await neo4jDriver.executeQuery(
        // language=Cypher
        `
            MATCH (jim:Person {name: 'Jim Hubbard'})
            MATCH (sarah:Person {name: 'Sarah Schulman'})
            WITH jim, sarah
            CREATE (i:Interview {number: $interviewNumber, date: date($date), uid: $interviewUID, url: $interviewURL})
            CREATE (video:Video {url: $videoURL, uid: $videoUID})
            CREATE (interviewee:Person {name: $interviewee, uid: $intervieweeUID})
            CREATE (sarahSpeaker:Speaker:Interviewer {label: $speakers.sarah})
            CREATE (jimSpeaker:Speaker:Interviewer {label: $speakers.jim})
            CREATE (intervieweeSpeaker:Speaker:Interviewee {label: $speakers.interviewee})
            MERGE (interviewee)-[:INTERVIEWED_AS]->(intervieweeSpeaker)
            MERGE (jim)-[:INTERVIEWED_AS]->(jimSpeaker)
            MERGE (sarah)-[:INTERVIEWED_AS]->(sarahSpeaker)
            CREATE (i)-[:INTERVIEWED_WITH]->(intervieweeSpeaker)
            CREATE (i)-[:INTERVIEWED_WITH]->(jimSpeaker)
            CREATE (i)-[:INTERVIEWED_WITH]->(sarahSpeaker)
            CREATE (i)-[:HAS_VIDEO]->(video)
        `, {
            date,
            interviewee,
            intervieweeUID: nanoid(),
            interviewNumber: int(interviewNumber),
            interviewUID: nanoid(),
            interviewURL: `/${ interviewNumber }`,
            speakers,
            videoUID: nanoid(),
            videoURL,
        },
    )

    const segments = data.results.speaker_labels.segments.map((segment) => {
        const text = segment.items.reduce((acc, item) => {
            const itemIndex = data.results.items.findIndex(
                el => el.start_time === item.start_time,
            )
            const word = data.results.items[itemIndex]
            acc += ' ' + word.alternatives[0].content

            const nextWord = data.results.items[itemIndex + 1]
            if (nextWord?.type === 'punctuation') {
                acc += nextWord.alternatives[0].content
            }

            return acc
        }, '')

        const startTime = round(Number.parseFloat(segment.start_time), 2)
        const endTime = round(Number.parseFloat(segment.end_time), 2)
        const duration = round(endTime - startTime, 2)

        return {
            duration,
            endTime,
            speaker: segment.speaker_label,
            startTime,
            text: text.trim(),
        }
    })

    // await fs.writeFile('segments.json', JSON.stringify(segments, null, 4))

    for await (let segment of segments) {
        const result: EagerResult = await neo4jDriver.executeQuery(
            // language=Cypher
            `
                MATCH (speaker:Speaker {label: $speaker}) <-[:INTERVIEWED_WITH]- (interview:Interview {number: $interviewNumber})
                MATCH (speaker) <-[:INTERVIEWED_AS]- (person:Person)
                CREATE (statement:Statement {text: $text})
                MERGE (speaker)-[:SAYS {startTime: $startTime, endTime: $endTime, duration: $duration}]->(statement)
                RETURN statement, person, interview
            `, {
                ...segment,
                interviewNumber: int(interviewNumber),
            })

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
        await neo4jDriver.executeQuery(
            // language=Cypher
            `
                CREATE CONSTRAINT ${ label }UID IF NOT EXISTS
                FOR (n:${ label }) REQUIRE n.uid IS UNIQUE
            `,
            { label },
        )
    }

    await neo4jDriver.executeQuery(
        // language=Cypher
        `
            CREATE FULLTEXT INDEX transcript_search IF NOT EXISTS
            FOR (n:Statement) ON EACH [n.text]
        `)

    await neo4jDriver.executeQuery(
        // language=Cypher
        `
            CREATE (jim:Person {name: 'Jim Hubbard', uid: $jimUID})
            CREATE (sarah:Person {name: 'Sarah Schulman', uid: $sarahUID})
        `, { jimUID: nanoid(), sarahUID: nanoid() },
    )
}

async function main() {
    await bootstrap()
    let data


    // ~~~~~~~~~~~~~~~~~~~~~~ 002 - Robert Vasquez-Pacheco ~~~~~~~~~~~~~~~~~~~~~~ //
    data = JSON.parse(
        await fs.readFile(
            path.join(__dirname, '../assets/testing/002_robert_vasquez-pacheco.json'),
            'utf8',
        ),
    )
    await seed({
        data,
        date: '2002-12-14',
        interviewee: 'Robert Vasquez-Pacheco',
        interviewNumber: 2,
        speakers: {
            interviewee: 'spk_1',
            jim: 'spk_2',
            sarah: 'spk_0',
        },
        videoURL: '/interviews/002_robert_vasquez-pacheco.mp4',
    })
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~ //

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~ 003 - Moises Agosto ~~~~~~~~~~~~~~~~~~~~~~~~~~ //
    data = JSON.parse(
        await fs.readFile(
            path.join(__dirname, '../assets/testing/003_moises_agosto.json'),
            'utf8',
        ),
    )
    await seed({
        data,
        date: '2002-12-15',
        interviewee: 'Moises Agosto',
        interviewNumber: 3,
        speakers: {
            interviewee: 'spk_1',
            jim: 'spk_2',
            sarah: 'spk_0',
        },
        videoURL: '/interviews/003_moises_agosto.mp4',
    })
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~ //

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~ 012 - Mark Harrington ~~~~~~~~~~~~~~~~~~~~~~~~~ //
    data = JSON.parse(
        await fs.readFile(
            path.join(__dirname, '../assets/testing/012_mark_harrington.json'),
            'utf8',
        ),
    )
    await seed({
        data,
        date: '2002-12-16',
        interviewee: 'Mark Harrington',
        interviewNumber: 12,
        speakers: {
            interviewee: 'spk_0',
            jim: 'spk_2',
            sarah: 'spk_1',
        },
        videoURL: '/interviews/012_mark_harrington.mp4',
    })
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~ //

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~ 025 - Lei Chou ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~ //
    data = JSON.parse(
        await fs.readFile(
            path.join(__dirname, '../assets/testing/025_lei_chou.json'),
            'utf8',
        ),
    )
    await seed({
        data,
        date: '2003-05-15',
        interviewee: 'Lei Chou',
        interviewNumber: 25,
        speakers: {
            interviewee: 'spk_1',
            jim: 'spk_2',
            sarah: 'spk_0',
        },
        videoURL: '/interviews/025_lei_chou.mp4',
    })
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~ //

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~ 026 - Iris Long ~~~~~~~~~~~~~~~~~~~~~~~~~~~~ //
    data = JSON.parse(
        await fs.readFile(
            path.join(__dirname, '../assets/testing/026_iris_long.json'),
            'utf8',
        ),
    )
    await seed({
        data,
        date: '2003-05-16',
        interviewee: 'Iris Long',
        interviewNumber: 26,
        speakers: {
            interviewee: 'spk_1',
            jim: 'spk_0',
            sarah: 'spk_2',
        },
        videoURL: '/interviews/026_iris_long.mp4',
    })
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~ //

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~ 035 - Larry Kramer ~~~~~~~~~~~~~~~~~~~~~~~~~~~ //
    data = JSON.parse(
        await fs.readFile(
            path.join(__dirname, '../assets/testing/035_larry_kramer.json'),
            'utf8',
        ),
    )
    await seed({
        data,
        date: '2003-11-15',
        interviewee: 'Larry Kramer',
        interviewNumber: 35,
        speakers: {
            interviewee: 'spk_2',
            jim: 'spk_1',
            sarah: 'spk_0',
        },
        videoURL: '/interviews/035_larry_kramer.mp4',
    })
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~ //

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~ 074 - Douglas Crimp ~~~~~~~~~~~~~~~~~~~~~~~~~~ //
    data = JSON.parse(
        await fs.readFile(
            path.join(__dirname, '../assets/testing/074_douglas_crimp.json'),
            'utf8',
        ),
    )
    await seed({
        data,
        date: '2004-05-15',
        interviewee: 'Douglas Crimp',
        interviewNumber: 74,
        speakers: {
            interviewee: 'spk_1',
            jim: 'spk_2',
            sarah: 'spk_0',
        },
        videoURL: '/interviews/074_douglas_crimp.mp4',
    })
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~ //


    await neo4jDriver.close()
}

void (await main())
