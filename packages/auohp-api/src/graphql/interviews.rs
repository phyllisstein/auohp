use async_graphql::{Context, SimpleObject};
use neo4rs::query;

use crate::neo4j::Db;

// Each struct below mirrors one node label in the Neo4j graph.
// The mapping is deliberately one-to-one so that changes to the graph model
// surface immediately as compilation errors here, rather than silently
// surviving as stale projection queries.
//
// Relationships with properties (TRANSCRIBES) become intermediate GraphQL
// types (TranscriptEntry), since GraphQL has no native concept of a
// property-bearing edge. Relationships without properties (HAS_TRANSCRIPT,
// SAYS, INTERVIEWED_AS) become nested fields on the source type.

/// Mirrors the (:Person) node.
#[derive(SimpleObject)]
pub struct Person {
    pub uid: String,
    pub name: String,
}

/// Mirrors the (:Speaker) node — the role a Person plays in a given interview.
///
/// Note: the Speaker indirection is acknowledged as clumsy in CLAUDE.md.
/// The planned future model is (:Statement)<-[:SAYS]-(:Person) directly.
/// This type will be removed when that migration happens.
#[derive(SimpleObject)]
pub struct Speaker {
    /// "Interviewer" or "Interviewee".
    pub label: String,
    /// The Person who occupies this speaker role (via INTERVIEWED_AS).
    pub person: Person,
}

/// Mirrors the (:Statement) node.
#[derive(SimpleObject)]
pub struct Statement {
    pub uid: String,
    pub text: String,
    /// The speaker who uttered this statement (via SAYS → INTERVIEWED_AS).
    pub speaker: Speaker,
}

/// Represents the [TRANSCRIBES] relationship and its target Statement.
///
/// In GraphQL, a relationship with properties has no direct equivalent, so
/// we promote it to a first-class type. `TranscriptEntry` carries the timing
/// data that lives on the TRANSCRIBES edge plus a reference to the Statement
/// node that edge points at.
#[derive(SimpleObject)]
pub struct TranscriptEntry {
    /// Seconds from the start of the recording.
    pub start_time: f64,
    pub end_time: f64,
    /// Human-readable timestamp string, e.g. "00:04:32.500".
    pub start_timestamp: String,
    pub end_timestamp: String,
    pub statement: Statement,
}

/// Mirrors the (:Transcript) node, with its ordered entries and the
/// Interview it belongs to.
#[derive(SimpleObject)]
pub struct Transcript {
    pub uid: String,
    /// The interview this transcript belongs to (via HAS_TRANSCRIPT).
    pub interview: Interview,
    /// Statements in chronological order (sorted by TRANSCRIBES.startTime).
    pub entries: Vec<TranscriptEntry>,
}

/// Mirrors the (:Interview) node.
#[derive(SimpleObject)]
pub struct Interview {
    pub uid: String,
    pub number: i64,
    pub interviewee: String,
    /// ISO 8601 date string, e.g. "1995-04-23".
    pub date: String,
}

// ---------------------------------------------------------------------------
// Error helper
// ---------------------------------------------------------------------------

// neo4rs and async-graphql use different error types. This helper converts
// any Display value into an async_graphql::Error so we can use .map_err()
// throughout without repeating the conversion inline.
fn gql_err(e: impl std::fmt::Display) -> async_graphql::Error {
    async_graphql::Error::new(e.to_string())
}

// ---------------------------------------------------------------------------
// Resolver functions — called from QueryRoot in mod.rs.
// ---------------------------------------------------------------------------

pub async fn list_interviews(ctx: &Context<'_>) -> async_graphql::Result<Vec<Interview>> {
    // ctx.data::<T>() extracts a value that was embedded in the schema via
    // Schema::build(...).data(value). Returning Err here propagates as a
    // GraphQL error to the client — the server keeps running.
    let db = ctx.data::<Db>()?;

    let mut stream = db
        .execute(query(
            // toString() converts the Neo4j Date type to an ISO 8601 string
            // so we can return it as a plain Rust String.
            "MATCH (interview:Interview)
             RETURN interview.uid        AS uid,
                    interview.number     AS number,
                    interview.interviewee AS interviewee,
                    toString(interview.date) AS date
             ORDER BY interview.date ASC",
        ))
        .await
        .map_err(gql_err)?;

    let mut interviews = Vec::new();

    // RowStream::next() is an async method, not the Iterator trait, so we
    // use a while-let loop rather than for-in. Each call fetches the next
    // row over the Bolt connection.
    while let Some(row) = stream.next().await.map_err(gql_err)? {
        interviews.push(Interview {
            uid: row.get("uid").map_err(gql_err)?,
            number: row.get("number").map_err(gql_err)?,
            interviewee: row.get("interviewee").map_err(gql_err)?,
            date: row.get("date").map_err(gql_err)?,
        });
    }

    Ok(interviews)
}

pub async fn get_transcript(
    ctx: &Context<'_>,
    number: i64,
) -> async_graphql::Result<Transcript> {
    let db = ctx.data::<Db>()?;

    let mut stream = db
        .execute(
            // A single query that traverses the full path from Interview to
            // Person and returns everything we need to build the response.
            // The result set has one row per Statement; interview/transcript
            // columns repeat on every row (we only read them once below).
            query(
                "MATCH (interview:Interview {number: $number})-[:HAS_TRANSCRIPT]->(transcript:Transcript)
                 MATCH (transcript)-[transcribes:TRANSCRIBES]->(statement:Statement)
                       <-[:SAYS]-(speaker)
                       <-[:INTERVIEWED_AS]-(person:Person)
                 RETURN interview.uid         AS interview_uid,
                        interview.number      AS interview_number,
                        interview.interviewee AS interviewee,
                        toString(interview.date) AS interview_date,
                        transcript.uid        AS transcript_uid,
                        transcribes.startTime AS start_time,
                        transcribes.endTime   AS end_time,
                        transcribes.startTimestamp AS start_timestamp,
                        transcribes.endTimestamp   AS end_timestamp,
                        statement.uid         AS statement_uid,
                        statement.text        AS statement_text,
                        speaker.label         AS speaker_label,
                        person.uid            AS person_uid,
                        person.name           AS person_name
                 ORDER BY transcribes.startTime",
            )
            .param("number", number),
        )
        .await
        .map_err(gql_err)?;

    let mut interview_opt: Option<Interview> = None;
    let mut transcript_uid = String::new();
    let mut entries: Vec<TranscriptEntry> = Vec::new();

    while let Some(row) = stream.next().await.map_err(gql_err)? {
        // Interview and Transcript metadata repeat on every row; we only
        // need to extract it once.
        if interview_opt.is_none() {
            interview_opt = Some(Interview {
                uid: row.get("interview_uid").map_err(gql_err)?,
                number: row.get("interview_number").map_err(gql_err)?,
                interviewee: row.get("interviewee").map_err(gql_err)?,
                date: row.get("interview_date").map_err(gql_err)?,
            });
            transcript_uid = row.get("transcript_uid").map_err(gql_err)?;
        }

        // One row = one TranscriptEntry, its Statement, Speaker, and Person.
        entries.push(TranscriptEntry {
            start_time: row.get("start_time").map_err(gql_err)?,
            end_time: row.get("end_time").map_err(gql_err)?,
            start_timestamp: row.get("start_timestamp").map_err(gql_err)?,
            end_timestamp: row.get("end_timestamp").map_err(gql_err)?,
            statement: Statement {
                uid: row.get("statement_uid").map_err(gql_err)?,
                text: row.get("statement_text").map_err(gql_err)?,
                speaker: Speaker {
                    label: row.get("speaker_label").map_err(gql_err)?,
                    person: Person {
                        uid: row.get("person_uid").map_err(gql_err)?,
                        name: row.get("person_name").map_err(gql_err)?,
                    },
                },
            },
        });
    }

    let interview = interview_opt.ok_or_else(|| {
        async_graphql::Error::new(format!("interview #{number} not found"))
    })?;

    Ok(Transcript {
        uid: transcript_uid,
        interview,
        entries,
    })
}
