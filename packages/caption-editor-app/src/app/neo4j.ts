'use server'
import {lightFormat} from 'date-fns'

import neo4j, {type Driver, int} from 'neo4j-driver'
import * as R from 'ramda'

export interface TranscriptChild {
  endTime: number
  endTimestamp: string
  startTime: number
  startTimestamp: string
  speaker: string
  speakerName: string
  type: string
  transcription: string
  uid: string
  children: Array<{text: string}>
}
export interface Neo4jResult {
  type: string
  interviewNumber: number
  uid: string
  children: TranscriptChild[]
}

export interface Interview {
  number: number
  uid: string
  date: Date
  title: string
}

const NEO4J_URI = process?.env?.NEO4J_URI || 'neo4j://neo4j:7687'
const NEO4J_USER = process?.env?.NEO4J_USER || 'neo4j'
const NEO4J_PASSWORD = process?.env?.NEO4J_PASSWORD || 'auohpauohp'

let driver: Driver | null = null
export async function connect(
  uri: string = NEO4J_URI,
  username: string = NEO4J_USER,
  password: string = NEO4J_PASSWORD,
): Promise<Driver> {
  driver = neo4j.driver(
    uri,
    neo4j.auth.basic(username, password),
    {
      disableLosslessIntegers: true,
    },
  )

  try {
    const serverInfo = await driver.getServerInfo()
    console.log(`Connected to ${serverInfo.address}`)
  } catch (err) {
    console.error('Failed to connect to Neo4j')
    console.error(err)
    throw new Error('Failed to connect to Neo4j')
  }

  return driver
}

export async function disconnect() {
  if (!driver) {
    return
  }

  await driver.close()
  console.log('Disconnected from Neo4j')
}

export async function updateTranscriptStatement(attributes: Partial<TranscriptChild>) {
  const driver = await connect()

  if (!attributes.uid) {
    throw new Error('Missing uid')
  }

  if (!Array.isArray(attributes.children) || attributes.children.length === 0) {
    throw new Error('Missing children')
  }

  try {
    const transcription = R.path(['children', 0, 'text'], attributes)
    const statementMeta = R.pick(['startTime', 'endTime', 'startTimestamp', 'endTimestamp'], attributes)
    const speaker = R.pick(['speaker'], attributes)
    const person = R.pick(['speakerName'], attributes)
    const statement = R.assoc('text', transcription, R.pick(['uid'], attributes))

    console.log({statement, statementMeta, speaker, person})

    const result = await driver.executeQuery(
      // language=Cypher
      `
        MATCH (statement:Statement {uid: $statement.uid}) <-[statementMeta:TRANSCRIBES]-(transcript:Transcript)
        MATCH (statement) <-[:SAYS]-(speaker)<-[:INTERVIEWED_AS]-(person)
        SET statement += $statement
        SET statementMeta += $statementMeta
        SET speaker += $speaker
        SET person += $person
        RETURN transcript, statement, speaker, statementMeta, person
      `, {statement, statementMeta, speaker, person})

    if (result.records.length === 0) {
      throw new Error('Failed to update statement')
    }

    const record = result.records[0]
    const nextSpeaker = record.get('speaker')
    const nextStatement = record.get('statement')
    const nextStatementMeta = record.get('statementMeta')
    const nextPerson = record.get('person')

    const ret = {
      endTime: nextStatementMeta.properties.endTime,
      endTimestamp: nextStatementMeta.properties.endTimestamp,
      startTime: nextStatementMeta.properties.startTime,
      startTimestamp: nextStatementMeta.properties.startTimestamp,
      speaker: nextSpeaker.properties.label,
      speakerName: nextPerson.properties.name,
      type: 'statement',
      transcription: nextStatement.properties.text,
      uid: nextStatement.properties.uid,
      children: [
        {
          text: nextStatement.properties.text,
        },
      ],
    }

    console.log(ret)

    return ret
  } catch (err) {
    console.error(err)
    throw new Error('Failed to update transcript')
  }
}

export async function getInterviewTranscript(interviewNumber: number): Promise<Neo4jResult> {
  const driver = await connect()

  try {
    const transcriptMeta = await driver.executeQuery(
      `
        MATCH (interview:Interview {number: $interviewNumber})-[:HAS_TRANSCRIPT]->(transcript:Transcript)
        RETURN interview, transcript
        LIMIT 1
      `, {interviewNumber: int(interviewNumber)})

    if (!Array.isArray(transcriptMeta.records) || transcriptMeta.records.length === 0) {
      console.error('Transcript not found')
      throw new Error('Transcript not found')
    }

    const meta = transcriptMeta.records[0]
    const metadata = {
      transcript: meta.get('transcript'),
      interview: meta.get('interview'),
    }

    const rawTranscript = await driver.executeQuery(
      // language=Cypher
      `
        MATCH (transcript:Transcript {uid: $uid})
        MATCH (transcript) -[statementMeta:TRANSCRIBES]->(statement)<-[:SAYS]-(speaker)<-[:INTERVIEWED_AS]-(person)
        RETURN transcript, statement, speaker, statementMeta, person
        ORDER BY statementMeta.startTime
      `, {uid: metadata.transcript.properties.uid})

    const transcriptChildren: TranscriptChild[] = rawTranscript.records.map(record => {
      const speaker = record.get('speaker')
      const statement = record.get('statement')
      const statementMeta = record.get('statementMeta')
      const person = record.get('person')

      return {
        endTime: statementMeta.properties.endTime,
        endTimestamp: statementMeta.properties.endTimestamp,
        startTime: statementMeta.properties.startTime,
        startTimestamp: statementMeta.properties.startTimestamp,
        speaker: speaker.properties.label,
        speakerName: person.properties.name,
        type: 'statement',
        transcription: statement.properties.text,
        uid: statement.properties.uid,
        children: [
          {
            text: statement.properties.text,
          },
        ],
      }
    })
    const transcriptJSON = {
      type: 'transcript',
      interviewNumber: metadata.interview?.number,
      uid: metadata.interview?.uid,
      children: transcriptChildren,
    }
    return transcriptJSON
  } catch (err) {
    console.error(err)
    throw new Error('Failed to get transcript')
  }
}

export async function listInterviews(): Promise<Transcript[]> {
  const driver = await connect()

  try {
    const result = await driver.executeQuery(`
      MATCH (interview:Interview)
      RETURN interview
    `)

    return result.records.map(record => {
      const interview = record.get('interview')
      return {
        number: interview.properties.number,
        uid: interview.properties.uid,
        date: lightFormat(
          interview.properties.date.toStandardDate(),
          'yyyy-MM-dd',
        ),
        title: interview.properties.title,
      }
    }).sort((a, b) => a.date.localeCompare(b.date))
  } catch (err) {
    console.error(err)
    throw new Error('Failed to list interviews')
  }
}
