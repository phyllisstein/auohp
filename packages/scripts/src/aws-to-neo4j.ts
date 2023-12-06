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

async function main () {
  const data = JSON.parse(
    await fs.readFile(
      path.join(__dirname, '../assets/035_larry_kramer.json'),
      'utf8',
    ),
  )

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

  await fs.writeFile(
    path.join(__dirname, '../assets/035_larry_kramer_segments.json'),
    JSON.stringify(segments, null, 2),
  )

  const driver = neo4j.driver(
    'bolt://localhost:7687',
    neo4j.auth.basic('neo4j', 'auohpauohp'),
  )

  await driver.getServerInfo()

  await driver.executeQuery(`
    MATCH p=()--()
    DETACH DELETE p
  `)
  await driver.executeQuery(`
    CREATE (i:Interview {number: 35, date: date('2003-11-15')})
    CREATE (larry:Person {name: 'Larry Kramer'})
    CREATE (sarah:Person {name: 'Sarah Schulman'})
    CREATE (jim:Person {name: 'Jim Hubbard'})
    CREATE (jimSpeaker:Speaker {remoteID: "spk_0"})
    CREATE (larrySpeaker:Speaker {remoteID: "spk_2"})
    CREATE (sarahSpeaker:Speaker {remoteID: "spk_1"})
    CREATE (larry)-[:INTERVIEWED_AS]->(larrySpeaker)
    CREATE (sarah)-[:INTERVIEWED_AS]->(sarahSpeaker)
    CREATE (jim)-[:INTERVIEWED_AS]->(jimSpeaker)
    CREATE (i)-[:INTERVIEWED_WITH]->(larrySpeaker)
    CREATE (i)-[:INTERVIEWED_WITH]->(sarahSpeaker)
    CREATE (i)-[:INTERVIEWED_WITH]->(jimSpeaker)
  `)

  for await (const segment of segments) {
    await driver.executeQuery(
      `
      CREATE (s:Statement {text: $text})
      WITH s
      MATCH (speaker:Speaker {remoteID: $speakerID})
      MERGE (speaker)-[:SAYS {startTime: $startTime, endTime: $endTime}]->(s)
    `,
      {
        text: segment.text,
        speakerID: segment.speaker,
        startTime: segment.startTime,
        endTime: segment.endTime,
      },
    )
  }

  await driver.close()
}

void (await main())
