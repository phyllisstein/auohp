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

    // RETURN i — returns the whole Interview node, not individual properties.
    // The Rust deserialization happens in Interview::from_node(), which reads
    // the node's properties by their real Neo4j names (uid, number, etc.).
    let mut stream = db
        .execute(query(
            "MATCH (i:Interview)
             RETURN i
             ORDER BY i.date ASC",
        ))
        .await
        .map_err(gql_err)?;

    let mut interviews = Vec::new();

    while let Some(row) = stream.next().await.map_err(gql_err)? {
        // row.get::<neo4rs::Node>("i") extracts the Bolt Node from the row.
        // The "i" here is the Cypher variable name — the only alias in the
        // entire query, and it matches what the query actually returns.
        let node: neo4rs::Node = row.get("i").map_err(gql_err)?;
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
    // RETURN now uses whole entities:
    //   i  — Interview node
    //   t  — Transcript node
    //   s  — Statement node
    //   p  — Person node (via :SAYS)
    //   c  — :CONTAINS relationship (carries startTime / endTime)
    //
    // The only scalar is `is_interviewer`, a boolean computed from the
    // graph pattern (does this Person have an :INTERVIEWED_BY edge from
    // this Interview?). That can't come from any single node or
    // relationship, so it stays as a column alias.
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

                 RETURN i, t, s, p, c,
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
            let i: neo4rs::Node = row.get("i").map_err(gql_err)?;
            let t: neo4rs::Node = row.get("t").map_err(gql_err)?;
            interview_opt = Some(Interview::from_node(&i)?);
            transcript_uid = t.get("uid").map_err(gql_err)?;
        }

        // Extract the Statement node, Person node, and :CONTAINS relationship.
        let s: neo4rs::Node = row.get("s").map_err(gql_err)?;
        let p: neo4rs::Node = row.get("p").map_err(gql_err)?;
        let c: neo4rs::Relation = row.get("c").map_err(gql_err)?;

        // Person can be deserialized directly — its fields match the node
        // properties exactly (uid, name).
        let person: Person = p.to().map_err(gql_err)?;

        statements.push(Statement {
            uid: s.get("uid").map_err(gql_err)?,
            text: s.get("text").map_err(gql_err)?,
            person,
            is_interviewer: row.get("is_interviewer").map_err(gql_err)?,
            // Timing lives on the :CONTAINS relationship, not the Statement node.
            // Relation::get() works just like Node::get() — it pulls a property
            // by name from the relationship's property map.
            start_time: c.get("startTime").map_err(gql_err)?,
            end_time: c.get("endTime").map_err(gql_err)?,
            words: s.get("words").map_err(gql_err)?,
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
