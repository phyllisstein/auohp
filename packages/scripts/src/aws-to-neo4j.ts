/**
 * Parse a JSON transcription produced by AWS Transcribe, assembling complete
 * spoken segments from the individual words.
 */

import * as fs from 'fs/promises'
import path from 'path'
import { fileURLToPath } from 'url'

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

const neo4jDriver = neo4j.driver(
    'bolt://localhost:7687',
    neo4j.auth.basic('neo4j', 'auohpauohp'),
    {
        disableLosslessIntegers: true,
    },
)

/**
 * Words into hunks (also borrowed from Premiere):
 *
 *    - Contiguous segments by the same speaker are merged.
 *    - Merged segments are split into hunks of around 20 seconds each.
 *    - Look for the end of a sentence or a pause in speech to split.
 *
 * Hunks are the smallest unit of storage, persisted directly into the
 * database. Edits to a hunk are assumed to occupy the same time frame as
 * the original hunk. (TK: Timestamp editing might be useful, but cumbersome
 * to cascade across many hunks.) This means that once a hunk has been
 * defined, it will not be split or merged. Captions will work along the
 * same assumptions: they follow hunk timestamps and are not split or
 * merged, even if their text becomes too long.
 */
function mergeContiguousSegments(segments: any[]) {
    return segments.reduce((acc, segment) => {
        const lastSegment = acc[acc.length - 1]
        if (lastSegment && lastSegment.speaker_label === segment.speaker_label) {
            lastSegment.end_time = segment.end_time
            lastSegment.items.push(...segment.items)
        } else {
            acc.push(segment)
        }
        return acc
    }, [])
}

function splitSegmentsIntoHunks(segments: any[]) {
    const hunks = []
    let hunk = { items: [], speaker_label: null, start_time: null, end_time: null }

    for (let segment of segments) {
        if (!hunk.speaker_label) {
            hunk.speaker_label = segment.speaker_label
            hunk.start_time = segment.start_time
        }

        hunk.items.push(...segment.items)

        if (segment.end_time - hunk.start_time > 20) {
            const lastItem = hunk.items[hunk.items.length - 1]
            const lastWord = lastItem.alternatives[0].content
            if (/\p{P}/u.test(lastWord)) {
                hunk.end_time = lastItem.end_time
                hunks.push(hunk)
                hunk = { items: [], speaker_label: null, start_time: null, end_time: null }
            }
        }
    }

async function seed({ data, date, interviewee, interviewNumber, speakers, videoURL }: any) {
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
            )

            if (word.type === 'punctuation') {
                return acc + word.alternatives[0].content
            }

            return acc + ' ' + word.alternatives[0].content
        }, '')

        const startTime =  Number.parseFloat(segment.start_time)
        const endTime = Number.parseFloat(segment.end_time)
        const duration = (endTime - startTime).toPrecision(2)

        return {
            duration,
            endTime,
            speaker: segment.speaker_label,
            startTime,
            statementUID: nanoid(),
            text: text.trim(),
        }
    })

    await neo4jDriver.executeQuery(
        // language=Cypher
        `
            MATCH (jim:Person {name: 'Jim Hubbard'})
            MATCH (sarah:Person {name: 'Sarah Schulman'})
            WITH jim, sarah
            CREATE (i:Interview {number: $interviewNumber, date: date($date), uid: $interviewUID})
            CREATE (video:Video {url: $videoURL, uid: $videoUID})
            CREATE (interviewee:Person {name: $interviewee, uid: $intervieweeUID})
            CREATE (sarahSpeaker:Speaker {label: $speakers.sarah})
            CREATE (jimSpeaker:Speaker {label: $speakers.jim})
            CREATE (intervieweeSpeaker:Speaker {label: $speakers.interviewee})
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
            speakers,
            videoUID: nanoid(),
            videoURL,
        },
    )

    for await (let segment of segments) {
        const params = {
            ...segment,
            interviewNumber: int(interviewNumber),
        }

        const result: EagerResult = await neo4jDriver.executeQuery(
            // language=Cypher
            `
                MATCH (speaker:Speaker {label: $speaker}) <-[:INTERVIEWED_WITH]- (interview:Interview {number: $interviewNumber})
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
        date: '2002-12-17',
        interviewee: 'Gregg Bordowitz',
        interviewNumber: 4,
        speakers: {
            interviewee: 'spk_1',
            jim: 'spk_0',
            sarah: 'spk_2',
        },
        videoURL: '/interviews/004_gregg_bordowitz.mp4',
    })
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~ //

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~ 026 - Iris Long ~~~~~~~~~~~~~~~~~~~~~~~~~~~~ //
    data = JSON.parse(
        await fs.readFile(
            path.join(__dirname, '../assets/026_iris_long.json'),
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
            path.join(__dirname, '../assets/035_larry_kramer.json'),
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

    await neo4jDriver.close()
}

void (await main())
