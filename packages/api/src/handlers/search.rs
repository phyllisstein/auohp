//! Handler for semantic search over Statement nodes.
//!
//! Route:
//!
//!   GET /search?q=<query>&limit=<n>  --- returns JSON array of SearchHit

use axum::{
    Json,
    extract::{Query, State},
    response::IntoResponse,
};
use neo4rs::{BoltType, query};
use serde::Deserialize;

use crate::AppState;
use crate::error::{AppError, internal};
use crate::models::{SearchHit, Statement, StatementNode};

// ---------------------------------------------------------------------------
// Query-string parameters
// ---------------------------------------------------------------------------

/// Typed query-string parameters for `GET /search`.
///
/// axum's `Query<T>` extractor deserializes `?q=...&limit=...` into this
/// struct automatically via `serde::Deserialize`. Any field that is
/// `Option<T>` is simply `None` when the key is absent from the URL.
#[derive(Debug, Deserialize)]
pub struct SearchParams {
    /// Natural-language query to embed and search for.
    pub q: String,
    /// Maximum number of results to return. Defaults to 15 if omitted.
    pub limit: Option<i64>,
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

/// Embeds `params.q` and returns the nearest Statement nodes ranked by
/// cosine similarity against the `statement_embedding` HNSW index.
pub async fn search_statements(
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
) -> Result<impl IntoResponse, AppError> {
    // ── Embed the query text (CPU-bound work) ─────────────────────────────
    //
    // `EmbedderHandle::embed` already dispatches to a blocking thread pool
    // internally, so we don't need spawn_blocking here --- but we do need to
    // clone the Arc so the closure can own it.
    let texts = vec![params.q.clone()];
    let vector: Vec<f32> = state
        .embedder
        .embed(texts)
        .await
        .map_err(internal)?
        .into_iter()
        .next()
        .ok_or_else(|| AppError::Internal("embedding produced no vectors".into()))?;

    // Convert Vec<f32> to Vec<BoltType> --- neo4j's vector procedures expect
    // a list of floats. We widen f32 --> f64 because neo4rs's BoltType::Float
    // wraps f64; the precision loss going back to f32 inside Neo4j is
    // irrelevant for ANN similarity ranking.
    let vector_bolt: Vec<BoltType> = vector.iter().map(|&v| BoltType::from(v as f64)).collect();

    let limit_val = params.limit.unwrap_or(15);

    // ── ANN index lookup + graph join ─────────────────────────────────────
    //
    // `db.index.vector.queryNodes` returns (node, score) pairs from the HNSW
    // index. We join outward to pull timing (from :CONTAINS), speaker (from
    // :SAYS), and interview context (via :HAS_TRANSCRIPT).
    let mut stream = state
        .db
        .execute(
            query(
                "
                    MATCH (statement:Statement)
                    SEARCH statement IN (
                        VECTOR INDEX statement_embedding
                        FOR $vector
                        LIMIT $limit
                    )

                    MATCH (transcript:Transcript)-[contains:CONTAINS]->(statement)
                        <-[:SAYS]-(person:Person)
                    MATCH (interview:Interview)-[:HAS_TRANSCRIPT]->(transcript)

                    RETURN interview, statement, person, contains
                ",
            )
            .param("limit", limit_val)
            .param("vector", vector_bolt),
        )
        .await
        .map_err(internal)?;

    let mut hits = Vec::new();

    while let Some(row) = stream.next().await.map_err(internal)? {
        let interview: neo4rs::Node = row.get("interview").map_err(internal)?;
        let statement: neo4rs::Node = row.get("statement").map_err(internal)?;
        let person: neo4rs::Node = row.get("person").map_err(internal)?;
        let contains: neo4rs::Relation = row.get("contains").map_err(internal)?;
        let sn: StatementNode = statement.to().map_err(internal)?;

        hits.push(SearchHit {
            statement: Statement {
                text: sn.text,
                person: person.to().map_err(internal)?,
                start_time: contains.get("startTime").map_err(internal)?,
                end_time: contains.get("endTime").map_err(internal)?,
                words: sn.words,
            },
            interview: interview.to().map_err(internal)?,
        });
    }

    Ok(Json(hits))
}
