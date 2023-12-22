/**
 * Parse a JSON transcription produced by AWS Transcribe, assembling complete
 * spoken segments from the individual words.
 */

import * as fs from 'fs/promises'
import path from 'path'
import { fileURLToPath } from 'url'

import neo4j from 'neo4j-driver'

const __filename = fileURLToPath(import.meta.url)
const __dirname = path.dirname(__filename)

const driver = neo4j.driver(
    'bolt://localhost:7687',
    neo4j.auth.basic('neo4j', 'auohpauohp'),
)

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
            startTime: segment.start_time,
            endTime: segment.end_time,
            speaker: segment.speaker_label,
            text: text.trim(),
        }
    })

    await driver.executeQuery(`
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
        await driver.executeQuery(`
            MATCH (speaker:Speaker {remoteID: $speakerID}) <-[:INTERVIEWED_WITH]- (interview:Interview {number: $interviewNumber})
            CREATE (s:Statement {text: $text})
            MERGE (speaker)-[:SAYS {startTime: $startTime, endTime: $endTime}]->(s)
        `, {
            text: segment.text,
            speakerID: segment.speaker,
            startTime: segment.startTime,
            endTime: segment.endTime,
            interviewNumber,
        })
    }
}

async function main () {
    await driver.getServerInfo()

    await driver.executeQuery(`
        MATCH p=()--()
        DETACH DELETE p
    `)

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

    await driver.close()
}

void (await main())
