use async_graphql::{Context, SimpleObject};
use neo4rs::query;
use serde::Deserialize;

use crate::neo4j::Db;
use super::error::gql_err;

// Each struct below mirrors one node label in the Neo4j graph. The mapping is
// deliberately close to the graph model so that schema changes surface as
// compilation errors. Relationships with properties (`:CONTAINS` timing) are
// represented as optional fields on the target type, since GraphQL has no
// native concept of a property-bearing edge.

/// Mirrors the (:Person) node.
///
/// Derives `Deserialize` so neo4rs can deserialize a Bolt Node directly into
/// this struct via `node.to::<Person>()`. The field names (`uid`, `name`)
/// match the Neo4j property names exactly, so no `#[serde(rename)]` is needed.
///
/// This is the idiomatic neo4rs pattern: return whole nodes from Cypher
/// (`RETURN p`) rather than destructuring properties into arbitrary column
/// aliases (`RETURN p.uid AS person_uid, p.name AS person_name`).
#[derive(SimpleObject, Clone, Deserialize)]
pub struct Person {
    pub uid: String,
    pub name: String,
}

/// Mirrors the (:Statement) node, with timing from the `:CONTAINS` edge and
/// speaker attribution from `:SAYS`.
#[derive(SimpleObject)]
pub struct Statement {
    pub uid: String,
    pub text: String,
    /// The person who said this (via `:SAYS`).
    pub person: Person,
    /// Whether this person was an interviewer in this interview (derived from
    /// the `:INTERVIEWED_BY` edge on the Interview node).
    pub is_interviewer: bool,
    /// Seconds from start of recording. Null for non-media statements
    /// (e.g. broadsheet text).
    pub start_time: Option<f64>,
    pub end_time: Option<f64>,
    /// Per-word timing data as a JSON string, e.g.
    /// `[{"word":"the","start":1.23,"end":1.45}, ...]`.
    /// Null if the transcription pipeline did not produce word-level timing.
    pub words: Option<String>,
}

/// Mirrors the (:Transcript) node, with its ordered statements and the
/// Interview it belongs to.
#[derive(SimpleObject)]
pub struct Transcript {
    pub uid: String,
    /// The interview this transcript belongs to (via `:HAS_TRANSCRIPT`).
    pub interview: Interview,
    /// Statements in transcript order (via the `:NEXT` linked list).
    pub statements: Vec<Statement>,
}

/// Mirrors the (:Interview) node.
#[derive(SimpleObject, Clone)]
pub struct Interview {
    pub uid: String,
    pub number: i64,
    pub interviewee: String,
    /// ISO 8601 date string, e.g. "1995-04-23".
    pub date: String,
}

// ---------------------------------------------------------------------------
// Resolver functions — called from QueryRoot in schema.rs.
// ---------------------------------------------------------------------------

pub async fn list_interviews(ctx: &Context<'_>) -> async_graphql::Result<Vec<Interview>> {
    let db = ctx.data::<Db>()?;

    let mut stream = db
        .execute(query(
            "MATCH (i:Interview)
             RETURN i.uid           AS uid,
                    i.number        AS number,
                    i.interviewee   AS interviewee,
                    toString(i.date) AS date
             ORDER BY i.date ASC",
        ))
        .await
        .map_err(gql_err)?;

    let mut interviews = Vec::new();

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

    // Walk the :NEXT linked list to return statements in transcript order.
    // The head of the list is the Statement with no inbound :NEXT edge from
    // another Statement in the same Transcript.
    let mut stream = db
        .execute(
            query(
                "MATCH (i:Interview {number: $number})-[:HAS_TRANSCRIPT]->(t:Transcript)

                 // Find the head of the linked list
                 MATCH (t)-[:CONTAINS]->(head:Statement)
                 WHERE NOT EXISTS {
                   MATCH (t)-[:CONTAINS]->(prev:Statement)-[:NEXT]->(head)
                 }

                 // Walk the :NEXT chain
                 MATCH path = (head)-[:NEXT*0..]->(s:Statement)
                 WHERE (t)-[:CONTAINS]->(s)
                 WITH i, t, s, length(path) AS pos

                 MATCH (t)-[c:CONTAINS]->(s)<-[:SAYS]-(p:Person)
                 OPTIONAL MATCH (i)-[:INTERVIEWED_BY]->(interviewer:Person)
                   WHERE interviewer = p

                 RETURN i.uid           AS interview_uid,
                        i.number        AS interview_number,
                        i.interviewee   AS interviewee,
                        toString(i.date) AS interview_date,
                        t.uid           AS transcript_uid,
                        s.uid           AS statement_uid,
                        s.text          AS statement_text,
                        c.startTime     AS start_time,
                        c.endTime       AS end_time,
                        s.words         AS words,
                        p.uid           AS person_uid,
                        p.name          AS person_name,
                        interviewer IS NOT NULL AS is_interviewer
                 ORDER BY pos",
            )
            .param("number", number),
        )
        .await
        .map_err(gql_err)?;

    let mut interview_opt: Option<Interview> = None;
    let mut transcript_uid = String::new();
    let mut statements: Vec<Statement> = Vec::new();

    while let Some(row) = stream.next().await.map_err(gql_err)? {
        if interview_opt.is_none() {
            interview_opt = Some(Interview {
                uid: row.get("interview_uid").map_err(gql_err)?,
                number: row.get("interview_number").map_err(gql_err)?,
                interviewee: row.get("interviewee").map_err(gql_err)?,
                date: row.get("interview_date").map_err(gql_err)?,
            });
            transcript_uid = row.get("transcript_uid").map_err(gql_err)?;
        }

        statements.push(Statement {
            uid: row.get("statement_uid").map_err(gql_err)?,
            text: row.get("statement_text").map_err(gql_err)?,
            person: Person {
                uid: row.get("person_uid").map_err(gql_err)?,
                name: row.get("person_name").map_err(gql_err)?,
            },
            is_interviewer: row.get("is_interviewer").map_err(gql_err)?,
            start_time: row.get("start_time").map_err(gql_err)?,
            end_time: row.get("end_time").map_err(gql_err)?,
            words: row.get("words").map_err(gql_err)?,
        });
    }

    let interview = interview_opt.ok_or_else(|| {
        async_graphql::Error::new(format!("interview #{number} not found"))
    })?;

    Ok(Transcript {
        uid: transcript_uid,
        interview,
        statements,
    })
}
