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
}

void (await main())
