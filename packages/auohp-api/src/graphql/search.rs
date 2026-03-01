//! Vector search over Statement embeddings.
//!
//! `search_statements` embeds the caller's query string on-device (BGE-small,
//! 384-dim) and queries the `statement_embedding` HNSW index in Neo4j, joining
//! back to the parent Interview for context. Results are ordered by cosine
//! similarity descending.

use std::sync::Arc;

use async_graphql::{Context, SimpleObject};
use neo4rs::{BoltType, query};

use crate::embeddings::Embedder;
use crate::neo4j::Db;
use super::error::gql_err;
use super::interviews::{Interview, Statement};

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
    .map_err(gql_err)?       // JoinError (spawn_blocking panicked)
    .map_err(gql_err)?       // anyhow::Error from the ONNX model
    .into_iter()
    .next()
    .ok_or_else(|| async_graphql::Error::new("embedding produced no vectors"))?;

    // Convert to BoltType list — Neo4j's vector procedures expect a list of
    // floats. We widen f32 → f64 here because neo4rs's BoltType::Float wraps
    // f64; the precision lost going back to f32 inside Neo4j is irrelevant.
    let vector_bolt: Vec<BoltType> = vector
        .iter()
        .map(|&v| BoltType::from(v as f64))
        .collect();

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
                 YIELD node AS s, score

                 MATCH (t:Transcript)-[c:CONTAINS]->(s)<-[:SAYS]-(p:Person)
                 MATCH (i:Interview)-[:HAS_TRANSCRIPT]->(t)
                 OPTIONAL MATCH (i)-[:INTERVIEWED_BY]->(interviewer:Person)
                   WHERE interviewer = p

                 RETURN i, s, p, c, score,
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
        let i: neo4rs::Node = row.get("i").map_err(gql_err)?;
        let s: neo4rs::Node = row.get("s").map_err(gql_err)?;
        let p: neo4rs::Node = row.get("p").map_err(gql_err)?;
        let c: neo4rs::Relation = row.get("c").map_err(gql_err)?;

        hits.push(SearchHit {
            score: row.get("score").map_err(gql_err)?,
            statement: Statement {
                uid: s.get("uid").map_err(gql_err)?,
                text: s.get("text").map_err(gql_err)?,
                person: p.to().map_err(gql_err)?,
                is_interviewer: row.get("is_interviewer").map_err(gql_err)?,
                start_time: c.get("startTime").map_err(gql_err)?,
                end_time: c.get("endTime").map_err(gql_err)?,
                words: s.get("words").map_err(gql_err)?,
            },
            interview: Interview::from_node(&i)?,
        });
    }

    Ok(hits)
}
