use async_graphql::{Context, SimpleObject};
use chrono::NaiveDate;
use neo4rs::query;
use serde::Deserialize;

use super::error::gql_err;
use crate::neo4j::Db;

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

/// The node-native properties of a (:Statement) node.
///
/// Separated from `Statement` because `Statement` mixes node properties with
/// data from relationships (`:CONTAINS` timing, `:SAYS` speaker), making a
/// full `Deserialize` derive impossible. This struct covers only what lives on
/// the node itself. `#[serde(default)]` on `words` means serde substitutes
/// `None` for absent keys rather than erroring---older transcripts were seeded
/// without word-level timing, so the property may not exist on the node at all.
#[derive(Deserialize)]
pub struct StatementNode {
    pub uid: String,
    pub text: String,
    #[serde(default)]
    pub words: Option<String>,
}

#[derive(Deserialize)]
pub struct StatementMeta {
    pub start_time: f64,
    pub end_time: f64,
}

/// Mirrors the (:Statement) node, with timing from the `:CONTAINS` edge and
/// speaker attribution from `:SAYS`.
#[derive(SimpleObject)]
pub struct Statement {
    pub text: String,
    /// The person who said this (via `:SAYS`).
    pub person: Person,
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
///
/// The `date` field is stored as a Neo4j Date and deserialized into
/// `chrono::NaiveDate`. async-graphql's `"chrono"` feature registers
/// NaiveDate as a GraphQL scalar that serializes to ISO 8601 strings
/// ("2003-05-05")---so the GraphQL API still returns a string, but
/// the Rust code works with a typed date value.
#[derive(SimpleObject, Clone, Deserialize)]
pub struct Interview {
    pub uid: String,
    pub number: i64,
    pub interviewee: String,
    pub date: NaiveDate,
}

// ---------------------------------------------------------------------------
// Resolver functions---called from QueryRoot in schema.rs.
// ---------------------------------------------------------------------------

pub async fn list_interviews(ctx: &Context<'_>) -> async_graphql::Result<Vec<Interview>> {
    let db = ctx.data::<Db>()?;

    let mut stream = db
        .execute(query(
            "MATCH (interview:Interview)
             RETURN interview
             ORDER BY interview.date ASC",
        ))
        .await
        .map_err(gql_err)?;

    let mut interviews = Vec::new();

    while let Some(row) = stream.next().await.map_err(gql_err)? {
        let node: neo4rs::Node = row.get("interview").map_err(gql_err)?;
        interviews.push(node.to().map_err(gql_err)?);
    }

    Ok(interviews)
}

pub async fn get_transcript(ctx: &Context<'_>, number: i64) -> async_graphql::Result<Transcript> {
    let db = ctx.data::<Db>()?;
    let mut stream = db
        .execute(
            query(
                "MATCH (interview:Interview {number: $number})
                       -[:HAS_TRANSCRIPT]->(transcript:Transcript)
                       -[contains:CONTAINS]->(statement:Statement)
                       <-[:SAYS]-(person:Person)
                 OPTIONAL MATCH (interview)-[:INTERVIEWED_BY]->(interviewer:Person)
                   WHERE interviewer = person

                 RETURN interview, transcript, statement, person, contains
                 ORDER BY contains.startTime",
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
            let node: neo4rs::Node = row.get("interview").map_err(gql_err)?;
            let t_node: neo4rs::Node = row.get("transcript").map_err(gql_err)?;
            interview_opt = Some(node.to().map_err(gql_err)?);
            transcript_uid = t_node.get("uid").map_err(gql_err)?;
        }

        let statement: neo4rs::Node = row.get("statement").map_err(gql_err)?;
        let person: neo4rs::Node = row.get("person").map_err(gql_err)?;
        let contains: neo4rs::Relation = row.get("contains").map_err(gql_err)?;
        let sn: StatementNode = statement.to().map_err(gql_err)?;

        statements.push(Statement {
            text: sn.text,
            // Person can be deserialized directly---its fields match the
            // node properties exactly (uid, name).
            person: person.to().map_err(gql_err)?,
            // Timing lives on the :CONTAINS relationship, not the Statement
            // node. Relation::get() works just like Node::get().
            start_time: contains.get("startTime").map_err(gql_err)?,
            end_time: contains.get("endTime").map_err(gql_err)?,
            words: sn.words,
        });
    }

    let interview = interview_opt
        .ok_or_else(|| async_graphql::Error::new(format!("interview #{number} not found")))?;

    Ok(Transcript {
        uid: transcript_uid,
        interview,
        statements,
    })
}
