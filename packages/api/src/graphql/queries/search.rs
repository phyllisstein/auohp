//! Vector search over Statement embeddings.
//!
//! `search_statements` embeds the caller's query string on-device
//! (nomic-embed-text-v1.5, 768-dim) and queries the `statementEmbedding`
//! HNSW index in Neo4j, joining back to the parent Interview for context.
//! Results are ordered by cosine similarity descending.

use std::sync::Arc;

use async_graphql::{Context, SimpleObject};
use neo4rs::{BoltType, query};

use crate::graphql::nodes::{Interview, Statement, StatementNode};
use crate::neo4j::Db;
use auohp_core::EmbedderHandle;
use serde::Deserialize;

// ---------------------------------------------------------------------------
// Output type
// ---------------------------------------------------------------------------

/// A single hit from a vector similarity search over Statement nodes.
#[derive(SimpleObject, Deserialize)]
pub struct SearchHit {
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
    let embedder = ctx.data::<Arc<EmbedderHandle>>()?;

    let texts = vec![query_text];
    // ── Embed query text (CPU-bound: run off the async executor) ──────────────
    let vector: Vec<f32> = embedder
        .embed(texts.clone())
        .await?
        .into_iter()
        .next()
        .ok_or_else(|| async_graphql::Error::new("embedding produced no vectors"))?;

    // Convert to BoltType list---Neo4j's vector procedures expect a list of
    // floats. We widen f32 --> f64 here because neo4rs's BoltType::Float wraps
    // f64; the precision lost going back to f32 inside Neo4j is irrelevant.
    let vector_bolt: Vec<BoltType> = vector.iter().map(|&v| BoltType::from(v as f64)).collect();
    let limit = limit.unwrap_or(15 as i64);

    // ── ANN index lookup + graph join ─────────────────────────────────────────
    //
    // db.index.vector.queryNodes returns (node, score) pairs from the HNSW
    // index. We immediately join outward to pull timing (from :CONTAINS),
    // speaker (from :SAYS), and interview context (via :HAS_TRANSCRIPT).
    let mut stream = db
        .execute(
            query(include_str!("./wrrf-search.cypher"))
                .param("query", texts.clone().join(" "))
                .param("queryVector", vector_bolt)
                .param("finalK", 50)
                .param("sourceK", 100)
                .param("limit", limit),
        )
        .await?;

    let mut hits = Vec::new();

    while let Some(row) = stream.next().await? {
        let interview: neo4rs::Node = row.get("interview")?;
        let statement: neo4rs::Node = row.get("statement")?;
        let person: neo4rs::Node = row.get("person")?;
        let span: neo4rs::Relation = row.get("span")?;
        let sn: StatementNode = statement.to()?;

        hits.push(SearchHit {
            statement: Statement {
                uid: sn.uid,
                text: sn.text,
                person: person.to()?,
                start_time: span.get("startTime")?,
                end_time: span.get("endTime")?,
                words: sn.words,
            },
            interview: interview.to()?,
        });
    }

    Ok(hits)
}

pub async fn search_interview(
    ctx: &Context<'_>,
    query_text: String,
    interview_uid: String,
) -> async_graphql::Result<Vec<SearchHit>> {
    let db = ctx.data::<Db>()?;
    let mut stream = db.execute(
        query(
            "
                CALL db.index.fulltext.queryNodes('statementText', $queryText) YIELD node AS statement, score
                WITH *
                MATCH (person) <-[:INTERVIEWS]- (interview:Interview {uid: $interviewUid}) -[:HAS_TRANSCRIPT]-> (t:Transcript) -[span:CONTAINS]-> (statement)
                WITH *, string.indexOf(toLower(statement.text), toLower($queryText)) AS start
                WHERE start <> -1
                RETURN statement,
                    interview,
                    person,
                    span
                ORDER BY span.startTime
            "
        )
        .param("queryText", query_text)
        .param("interviewUid", interview_uid),
    )
    .await?;

    let mut hits = Vec::new();
    while let Some(row) = stream.next().await? {
        let interview: neo4rs::Node = row.get("interview")?;
        let statement: neo4rs::Node = row.get("statement")?;
        let span: neo4rs::Relation = row.get("span")?;
        let person: neo4rs::Node = row.get("person")?;

        let statement_node: StatementNode = statement.to()?;

        hits.push(SearchHit {
            statement: Statement {
                uid: statement_node.uid,
                text: statement_node.text,
                person: person.to()?,
                start_time: span.get("startTime")?,
                end_time: span.get("endTime")?,
                words: None,
            },
            interview: interview.to()?,
        });
    }

    Ok(hits)
}
