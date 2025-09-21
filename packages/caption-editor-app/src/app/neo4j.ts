"use server";
import { lightFormat } from "date-fns";

import neo4j, { type Date as Neo4jDate, type Driver, int } from "neo4j-driver";
import * as R from "ramda";

export interface TranscriptChild {
    children: Array<{ text: string }>;
    endTime: number;
    endTimestamp: string;
    speaker: string;
    speakerName: string;
    startTime: number;
    startTimestamp: string;
    transcription: string;
    type: string;
    uid: string;
}

export interface InterviewTranscriptJSON {
    children: TranscriptChild[];
    interviewNumber: number;
    type: string;
    uid: string;
}

export interface Interview {
    date: Neo4jDate;
    interviewee: string;
    number: number;
    uid: string;
}

export interface Transcript {
    uid: string;
}

export interface Transcribes {
    endTime: number;
    endTimestamp: string;
    startTime: number;
    startTimestamp: string;
}

export interface Statement {
    text: string;
    uid: string;
}

// FIXME: Distinguish between Interviewer and Interviewee
export interface Interviewer {
    label: string;
}

// FIXME: Distinguish between Interviewee and Interviewer
export interface Interviewee {
    label: string;
}

export type Speaker = Interviewer | Interviewee;

export interface Person {
    name: string;
    uid: string;
}

export type WithProperties<T> = {
    [K in keyof T]: {
        properties: T[K];
    }
};

export interface Neo4jStatement {
    person: Person;
    speaker: Speaker;
    statement: Statement;
    transcribes: Transcribes;
}

export interface Neo4jInterview {
    interview: Interview;
    transcript: Transcript;
}

export interface Neo4jTranscript {
    interview: Interview;
    statements: Neo4jStatement[];
    transcript: Transcript;
}


const NEO4J_URI = process?.env?.NEO4J_URI || "neo4j://neo4j:7687";
const NEO4J_USER = process?.env?.NEO4J_USER || "neo4j";
const NEO4J_PASSWORD = process?.env?.NEO4J_PASSWORD || "auohpauohp";

let driver: Driver | null = null;
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
    );

    try {
        const serverInfo = await driver.getServerInfo();
        console.log(`Connected to ${ serverInfo.address }`);
    } catch (err) {
        console.error("Failed to connect to Neo4j");
        console.error(err);
        throw new Error("Failed to connect to Neo4j");
    }

    return driver;
}


export async function disconnect() {
    if (!driver) {
        return;
    }

    await driver.close();
    driver = null;
    console.log("Disconnected from Neo4j");
}


export async function getSession() {
    if (!driver) {
        await connect();
    }

    return driver.session();
}


export async function updateTranscriptStatement(attributes: Partial<TranscriptChild>) {
    const session = await getSession();

    if (!attributes.uid) {
        throw new Error("Missing uid");
    }

    if (!Array.isArray(attributes.children) || attributes.children.length === 0) {
        throw new Error("Missing children");
    }

    try {
        const transcription = R.path(["children", 0, "text"], attributes);
        const transcribes = R.pick(["startTime", "endTime", "startTimestamp", "endTimestamp"], attributes);
        const speaker = R.pick(["speaker"], attributes);
        const person = R.pick(["speakerName"], attributes);
        const statement = R.assoc("text", transcription, R.pick(["uid"], attributes));

        const result = await session.run<WithProperties<Neo4jStatement>>(
            // language=Cypher
            `
        MATCH (statement:Statement {uid: $statement.uid}) <-[transcribes:TRANSCRIBES]-(transcript:Transcript)
        MATCH (statement) <-[:SAYS]-(speaker)<-[:INTERVIEWED_AS]-(person)
        SET statement += $statement
        SET transcribes += transcribes
        SET speaker += $speaker
        SET person += $person
        RETURN transcript, statement, speaker, transcribes, person
      `, { person, speaker, statement, transcribes });

        if (result.records.length === 0) {
            throw new Error("Failed to update statement");
        }

        const record = result.records[0];
        const nextSpeaker = record.get("speaker");
        const nextStatement = record.get("statement");
        const nextTranscribes = record.get("transcribes");
        const nextPerson = record.get("person");

        const ret = {
            children: [
                {
                    text: nextStatement.properties.text,
                },
            ],
            endTime: nextTranscribes.properties.endTime,
            endTimestamp: nextTranscribes.properties.endTimestamp,
            speaker: nextSpeaker.properties.label,
            speakerName: nextPerson.properties.name,
            startTime: nextTranscribes.properties.startTime,
            startTimestamp: nextTranscribes.properties.startTimestamp,
            transcription: nextStatement.properties.text,
            type: "statement",
            uid: nextStatement.properties.uid,
        };

        console.log(ret);

        return ret;
    } catch (err) {
        console.error(err);
        throw new Error("Failed to update transcript");
    }
}


export async function getJSONInterviewTranscript(interviewNumber: number): Promise<InterviewTranscriptJSON> {
    const {
        interview,
        statements,
    } = await getInterviewTranscript(interviewNumber);

    const children: TranscriptChild[] = statements.map(statementRecord => {
        const {
            person,
            speaker,
            statement,
            transcribes,
        } = statementRecord;

        return {
            children: [
                {
                    text: statement.text,
                },
            ],
            endTime: transcribes.endTime,
            endTimestamp: transcribes.endTimestamp,
            speaker: speaker.label,
            speakerName: person.name,
            startTime: transcribes.startTime,
            startTimestamp: transcribes.startTimestamp,
            transcription: statement.text,
            type: "statement",
            uid: statement.uid,
        };
    });

    const ret = {
        children,
        interviewNumber: interview.number,
        type: "transcript",
        uid: interview.uid,
    };

    return ret;
}


export async function getVTTInterviewTranscript(interviewNumber: number): Promise<string> {
    const {
        statements,
    } = await getInterviewTranscript(interviewNumber);

    let vtt = "WEBVTT\n\n";

    for (const statementRecord of statements) {
        const {
            statement,
            transcribes,
        } = statementRecord;

        vtt += `${ transcribes.startTimestamp } --> ${ transcribes.endTimestamp }\n`;
        vtt += `${ statement.text }\n\n`;
    }

    return vtt;
}


export async function getInterviewTranscript(interviewNumber: number): Promise<Neo4jTranscript> {
    const session = await getSession();

    try {
        const transcriptMeta = await session.run<WithProperties<Neo4jInterview>>(
            `
        MATCH (interview:Interview {number: $interviewNumber})-[:HAS_TRANSCRIPT]->(transcript:Transcript)
        RETURN interview, transcript
        LIMIT 1
      `, { interviewNumber: int(interviewNumber) });

        if (!Array.isArray(transcriptMeta.records) || transcriptMeta.records.length === 0) {
            console.error(`Interview ${ interviewNumber } not found`);
            throw new Error(`Interview ${ interviewNumber } not found`);
        }

        const meta = transcriptMeta.records[0];
        const metadata = {
            interview: meta.get("interview").properties,
            transcript: meta.get("transcript").properties,
        };

        const rawTranscript = await session.run<WithProperties<Neo4jStatement>>(
            // language=Cypher
            `
        MATCH (transcript:Transcript {uid: $uid})
        MATCH (transcript) -[transcribes:TRANSCRIBES]->(statement)<-[:SAYS]-(speaker)<-[:INTERVIEWED_AS]-(person)
        RETURN transcript, statement, speaker, transcribes, person
        ORDER BY transcribes.startTime
      `, { uid: metadata.transcript.uid });

        const interviewTranscriptData = {
            interview: metadata.interview,
            statements: rawTranscript.records.map(record => {
                const speaker = record.get("speaker").properties;
                const statement = record.get("statement").properties;
                const transcribes = record.get("transcribes").properties;
                const person = record.get("person").properties;

                return {
                    person,
                    speaker,
                    statement,
                    transcribes,
                };
            }),
            transcript: metadata.transcript,
        };

        return interviewTranscriptData;
    } catch (err) {
        console.error(err);
        throw new Error("Failed to get transcript");
    }
}


export async function listInterviews(): Promise<Transcript[]> {
    const driver = await connect();

    try {
        const result = await driver.executeQuery(`
      MATCH (interview:Interview)
      RETURN interview
      ORDER BY interview.date ASC
    `);

        return result.records.map(record => {
            const interview = record.get("interview");
            return {
                date: lightFormat(
                    interview.properties.date.toStandardDate(),
                    "yyyy-MM-dd",
                ),
                number: interview.properties.number,
                title: interview.properties.title,
                uid: interview.properties.uid,
            };
        });
    } catch (err) {
        console.error(err);
        throw new Error("Failed to list interviews");
    }
}
