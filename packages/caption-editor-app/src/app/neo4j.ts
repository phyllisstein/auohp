'use server'

import neo4j, { type Driver, int } from 'neo4j-driver'


export interface Neo4jResult {
    endTime: number
    interviewNumber: number
    score: number
    speakerName: string
    startTime: number
    statement: string
    statementUID: string
    videoURL: string
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
    if (driver) {
        return driver
    }

    driver = neo4j.driver(
        uri,
        neo4j.auth.basic(username, password),
        {
            disableLosslessIntegers: true,
        },
    )

    try {
        const serverInfo = await driver.getServerInfo()
        console.log(`Connected to ${ serverInfo.address }`)
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

export async function updateTranscript(json) {
    const driver = await connect()

    try {
        for (const segment of json[0].children) {
            await driver.executeQuery(
                // language=Cypher
                `
                    MATCH (interview:Interview {number: $interviewNumber}) -[:HAS_VIDEO]-> (video)
                    MATCH (video)-[:HAS_TRANSCRIPT]-> (transcript)
                    MATCH (transcript) -[:INCLUDES_SPEAKER]- (speaker:Speaker {label: $speaker})
                    MATCH (statement) <-[:SAYS {startTime: $startTime, endTime: $endTime}]- (speaker)
                    SET statement.text = $transcription
                    RETURN statement
                `, {
                    ...segment,
                    interviewNumber: int(25),
                },
            )
        }
    } catch (err) {
        console.error('Failed to update transcript')
        console.error(err)
    }
}

export async function getTranscript(uid: string): Promise<Neo4jResult[]> {
    const driver = await connect()

    try {
        const transcriptMeta = await driver.executeQuery(
            `
            MATCH (transcript:Transcript {uid: 'CUtQCtNOJd2r2ezf9Cukb'})
            OPTIONAL MATCH (interview:Interview) -[:HAS_VIDEO]-> (video) -[:HAS_TRANSCRIPT]-> (transcript)
            OPTIONAL MATCH (documentary:Documentary) -[:HAS_VIDEO]-> (video) -[:HAS_TRANSCRIPT]-> (transcript)
            RETURN interview, documentary, transcript, video
            LIMIT 1
            `, { uid })

        const meta = transcriptMeta.records[0]
        if (!meta) {
            console.error('Transcript not found')
            throw new Error('Transcript not found')
        }
        const metadata = {
            interview: meta.get('interview').properties,
            documentary: meta.get('documentary').properties,
            transcript: meta.get('transcript').properties,
            video: meta.get('video').properties,
        }

        const rawTranscript = await driver.executeQuery(
            // language=Cypher
            `
                MATCH (transcript:Transcript {uid: $uid})
                MATCH (transcript) -[:INCLUDES_SPEAKER]-> (speaker:Speaker) -[speakerSays:SAYS]-> (statement)
                RETURN speakerSays, statement, speaker
                SORT BY speakerSays.startTime
            `, { uid })

        const transcriptChildren = rawTranscript.records.map(record => {
            const speaker = record.get('speaker')
            const statement = record.get('statement')
            const speakerSays = record.get('speakerSays')

            return {
                endTime: speakerSays.properties.endTime,
                endTimeStamp: speakerSays.properties.endTimeStamp,
                startTime: speakerSays.properties.startTime,
                startTimeStamp: speakerSays.properties.startTimeStamp,
                speaker: speaker.properties.label,
                type: 'segment',
                transcription: statement.properties.text,
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
            uid: metadata.interview?.uid || metadata.documentary?.uid,
            videoURL: metadata.video?.url,
            children: transcriptChildren,
        }
        return [transcriptJSON]
    } catch (err) {
        console.error('Failed to get transcript')
        console.error(err)
        throw new Error('Failed to get transcript')
    }
}
