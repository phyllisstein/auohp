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
)

async function seed({ data, date, interviewee, interviewNumber, speakers, videoURL }: any) {
    if (
        !Array.isArray(data?.results?.speaker_labels?.segments) ||
    data.results.speaker_labels.segments.length === 0
    ) {
        throw new Error('No speaker labels found')
    }

    /**
     * Premiere subtitle defaults:
     *     - Each subtitle must be less than 42 characters per line
     *     - Each subtitle must be no more than 2 lines
     *     - Each subtitle must be on screen for at least 3 seconds
     *
     * TODO: BBC has comprehensive guide to best practices:
     *     <https://www.bbc.co.uk/accessibility/forproducts/guides/subtitles/>
     */
    const segments = data.results.speaker_labels.segments.map(segment => {
        const text = segment.items.reduce((acc, item) => {
            const word = data.results.items.find(
                i => i.start_time === item.start_time,
            )?.alternatives[0].content
            return `${ acc } ${ word }`
        }, '')

        const startTime =  Number.parseInt(segment.start_time, 10)
        const endTime = Number.parseInt(segment.end_time, 10)
        const duration = endTime - startTime

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
