use async_graphql::{Context, SimpleObject};
use chrono::NaiveDate;
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

impl Interview {
    /// Construct an Interview from a neo4rs Node.
    ///
    /// We can't just `#[derive(Deserialize)]` on Interview because the
    /// `date` property is stored as a Neo4j Date (a typed temporal value)
    /// but our GraphQL schema exposes it as a String. neo4rs deserializes
    /// Bolt Dates into `chrono::NaiveDate`, so we pull it out explicitly
    /// and format it.
    ///
    /// The other three properties (uid, number, interviewee) are extracted
    /// with `node.get("property_name")` — the same API as Row::get, but
    /// operating on a Node's properties instead of row columns. The names
    /// here are the real Neo4j property names, not arbitrary Cypher aliases.
    pub fn from_node(node: &neo4rs::Node) -> async_graphql::Result<Self> {
        let date: NaiveDate = node.get("date").map_err(gql_err)?;
        Ok(Self {
            uid: node.get("uid").map_err(gql_err)?,
            number: node.get("number").map_err(gql_err)?,
            interviewee: node.get("interviewee").map_err(gql_err)?,
            date: date.to_string(),
        })
    }
}

// ---------------------------------------------------------------------------
// Resolver functions — called from QueryRoot in schema.rs.
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
        interviews.push(Interview::from_node(&node)?);
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
    //
    // The only scalar column is `is_interviewer` — a boolean computed from
    // the graph pattern that can't come from any single node or relationship.
    let mut stream = db
        .execute(
            query(
                "MATCH (interview:Interview {number: $number})
                       -[:HAS_TRANSCRIPT]->(transcript:Transcript)

                 // Find the head of the linked list
                 MATCH (transcript)-[:CONTAINS]->(head:Statement)
                 WHERE NOT EXISTS {
                   MATCH (transcript)-[:CONTAINS]->(prev:Statement)-[:NEXT]->(head)
                 }

                 // Walk the :NEXT chain
                 MATCH path = (head)-[:NEXT*0..]->(statement:Statement)
                 WHERE (transcript)-[:CONTAINS]->(statement)
                 WITH interview, transcript, statement, length(path) AS pos

                 MATCH (transcript)-[contains:CONTAINS]->(statement)
                       <-[:SAYS]-(person:Person)
                 OPTIONAL MATCH (interview)-[:INTERVIEWED_BY]->(interviewer:Person)
                   WHERE interviewer = person

                 RETURN interview, transcript, statement, person, contains,
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
            let node: neo4rs::Node = row.get("interview").map_err(gql_err)?;
            let t_node: neo4rs::Node = row.get("transcript").map_err(gql_err)?;
            interview_opt = Some(Interview::from_node(&node)?);
            transcript_uid = t_node.get("uid").map_err(gql_err)?;
        }

        let statement: neo4rs::Node = row.get("statement").map_err(gql_err)?;
        let person: neo4rs::Node = row.get("person").map_err(gql_err)?;
        let contains: neo4rs::Relation = row.get("contains").map_err(gql_err)?;

        statements.push(Statement {
            uid: statement.get("uid").map_err(gql_err)?,
            text: statement.get("text").map_err(gql_err)?,
            // Person can be deserialized directly — its fields match the
            // node properties exactly (uid, name).
            person: person.to().map_err(gql_err)?,
            is_interviewer: row.get("is_interviewer").map_err(gql_err)?,
            // Timing lives on the :CONTAINS relationship, not the Statement
            // node. Relation::get() works just like Node::get().
            start_time: contains.get("startTime").map_err(gql_err)?,
            end_time: contains.get("endTime").map_err(gql_err)?,
            words: statement.get("words").map_err(gql_err)?,
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
