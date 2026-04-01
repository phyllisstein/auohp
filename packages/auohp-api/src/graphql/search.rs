//! Vector search over Statement embeddings.
//!
//! `search_statements` embeds the caller's query string on-device
//! (nomic-embed-text-v1.5, 768-dim) and queries the `statement_embedding`
//! HNSW index in Neo4j, joining back to the parent Interview for context.
//! Results are ordered by cosine similarity descending.

use std::sync::Arc;

use async_graphql::{Context, SimpleObject};
use neo4rs::{query, BoltType};

use super::error::gql_err;
use super::interviews::{Interview, Statement, StatementNode};
use crate::embeddings::Embedder;
use crate::neo4j::Db;

// ---------------------------------------------------------------------------
// Output type
// ---------------------------------------------------------------------------

/// A single hit from a vector similarity search over Statement nodes.
#[derive(SimpleObject)]
pub struct SearchHit {
    /// Cosine similarity score in [0, 1]. Higher is more similar.
    pub score: f64,
    /// The matching statement, with speaker and timing.
    pub statement: Statement,
    /// The interview this statement belongs to.
    pub interview: Interview,
}

// ---------------------------------------------------------------------------
// Resolver
// ---------------------------------------------------------------------------

/// Embed `query_text` and return the `limit` nearest Statement nodes ranked by
/// cosine similarity. Defaults to 15 results if `limit` is not supplied.
pub async fn search_statements(
    ctx: &Context<'_>,
    query_text: String,
    limit: Option<i64>,
) -> async_graphql::Result<Vec<SearchHit>> {
    let db = ctx.data::<Db>()?;
    let embedder = ctx.data::<Arc<Embedder>>()?;

    // ── Embed query text (CPU-bound: run off the async executor) ──────────────
    let vector: Vec<f32> = tokio::task::spawn_blocking({
        let embedder = embedder.clone();
        let texts = vec![query_text];
        move || embedder.embed(&texts)
    })
    .await
    .map_err(gql_err)? // JoinError (spawn_blocking panicked)
    .map_err(gql_err)? // anyhow::Error from the ONNX model
    .into_iter()
    .next()
    .ok_or_else(|| async_graphql::Error::new("embedding produced no vectors"))?;

    // Convert to BoltType list---Neo4j's vector procedures expect a list of
    // floats. We widen f32 --> f64 here because neo4rs's BoltType::Float wraps
    // f64; the precision lost going back to f32 inside Neo4j is irrelevant.
    let vector_bolt: Vec<BoltType> = vector.iter().map(|&v| BoltType::from(v as f64)).collect();

    let limit_val = limit.unwrap_or(15);

    // ── ANN index lookup + graph join ─────────────────────────────────────────
    //
    // db.index.vector.queryNodes returns (node, score) pairs from the HNSW
    // index. We immediately join outward to pull timing (from :CONTAINS),
    // speaker (from :SAYS), and interview context (via :HAS_TRANSCRIPT).
    let mut stream = db
        .execute(
            query(
                "CALL db.index.vector.queryNodes('statement_embedding', $limit, $vector)
                 YIELD node AS statement, score

                 MATCH (transcript:Transcript)-[contains:CONTAINS]->(statement)
                       <-[:SAYS]-(person:Person)
                 MATCH (interview:Interview)-[:HAS_TRANSCRIPT]->(transcript)
                 OPTIONAL MATCH (interview)-[:INTERVIEWED_BY]->(interviewer:Person)
                   WHERE interviewer = person

                 RETURN interview, statement, person, contains, score,
                        interviewer IS NOT NULL AS is_interviewer
                 ORDER BY score DESC",
            )
            .param("limit", limit_val)
            .param("vector", vector_bolt),
        )
        .await
        .map_err(gql_err)?;

    let mut hits = Vec::new();

    while let Some(row) = stream.next().await.map_err(gql_err)? {
        let interview: neo4rs::Node = row.get("interview").map_err(gql_err)?;
        let statement: neo4rs::Node = row.get("statement").map_err(gql_err)?;
        let person: neo4rs::Node = row.get("person").map_err(gql_err)?;
        let contains: neo4rs::Relation = row.get("contains").map_err(gql_err)?;
        let sn: StatementNode = statement.to().map_err(gql_err)?;

        hits.push(SearchHit {
            score: row.get("score").map_err(gql_err)?,
            statement: Statement {
                uid: sn.uid,
                text: sn.text,
                person: person.to().map_err(gql_err)?,
                is_interviewer: row.get("is_interviewer").map_err(gql_err)?,
                start_time: contains.get("startTime").map_err(gql_err)?,
                end_time: contains.get("endTime").map_err(gql_err)?,
                words: sn.words,
            },
            interview: interview.to().map_err(gql_err)?,
        });
    }

    Ok(hits)
}
