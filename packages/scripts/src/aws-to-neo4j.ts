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

const neo4jDriver = neo4j.driver(
    'bolt://localhost:7687',
    neo4j.auth.basic('neo4j', 'auohpauohp'),
    {
        disableLosslessIntegers: true,
    },
)

function segmentIntoCaptions(segment: any) {
}

async function seed({ data, date, interviewee, interviewNumber, speakers, videoURL }: any) {
    if (
        !Array.isArray(data?.results?.speaker_labels?.segments) ||
        data.results.speaker_labels.segments.length === 0
    ) {
        throw new Error('No speaker labels found')
    }

    let currentSpeaker = data.results.items[0].speaker_label
    let currentCaption = [
        data.results.items[0],
    ]
    let captions = []
    for (let item = 0; item < data.results.items.length; item++) {
        const currentItem = data.results.items[item]
        if (currentItem.type === 'punctuation') {
            currentCaption.push(currentItem)
            continue
        }

        if (currentItem.speaker_label !== currentSpeaker) {
            captions.push(currentCaption)
            currentCaption = []
            currentSpeaker = currentItem.speaker_label
        }

        currentCaption.push(currentItem)
    }

    // const captions = segmentsIntoCaptions(segments)
    await fs.writeFile('segments.json', JSON.stringify(captions, null, 4))
    return

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
                CREATE (statement:Statement {text: $text})
                MERGE (speaker)-[:SAYS {startTime: $startTime, endTime: $endTime}]->(statement)
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


    // ~~~~~~~~~~~~~~~~~~~~~~ 002 - Robert Vasquez-Pacheco ~~~~~~~~~~~~~~~~~~~~~~ //
    let data = JSON.parse(
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
            interviewee: '',
            jim: '',
            sarah: '',
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
            interviewee: '',
            jim: '',
            sarah: '',
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
            interviewee: '',
            jim: '',
            sarah: '',
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
            interviewee: '',
            jim: '',
            sarah: '',
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
            interviewee: '',
            jim: '',
            sarah: '',
        },
        videoURL: '/interviews/074_douglas_crimp.mp4',
    })
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~ //


    await neo4jDriver.close()
}

void (await main())
