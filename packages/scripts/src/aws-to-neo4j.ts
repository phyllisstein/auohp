/**
 * The JSON file produced by Amazon Transcribe
 */

import * as fs from 'fs/promises'
import path from 'path'
import { fileURLToPath } from 'url'

import neo4j from 'neo4j-driver'

const __filename = fileURLToPath(import.meta.url)
const __dirname = path.dirname(__filename)

async  function main () {
  const data = JSON.parse(
    await fs.readFile(path.join(__dirname, '../assets/074_douglas_crimp.json'), 'utf8'),
  )

  const driver = neo4j.driver(
    'bolt://localhost:7687',
    neo4j.auth.basic('neo4j', 'auohp-phoua'),
    { disableLosslessIntegers: true },
  )
  const session = driver.session()

  if (
    !Array.isArray(!data?.results?.speaker_labels?.segments) ||
    data.results.speaker_labels.segments.length === 0
  ) {
    throw new Error('No speaker labels found')
  }

  const normalizedSegment = segment => {
    segment.items.reduce((acc, item) => {})
  }

  for await (const segment of data.results.speaker_labels.segments) {
    const segmnetText = segment
  }
}

void (await main())