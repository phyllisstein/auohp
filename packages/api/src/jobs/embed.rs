//! The embedding job: give an interview's statements their vectors.
//!
//! # What travels in the queue, and why it is so small
//!
//! [`EmbedInterview`] carries an interview uid and nothing else. That is a
//! deliberate correction to how the fire-and-forget version worked, which
//! moved `Vec<String>` of every statement's text into the spawned task.
//!
//! The argument is not just memory frugality. A queued task's arguments are
//! *serialized into a database row* and can sit there across a restart. Two
//! things follow:
//!
//! - Carrying transcripts would put an unbounded blob in every row, and N
//!   queued interviews would hold N transcripts resident whether or not any of
//!   them is running.
//! - More importantly, a payload captured at enqueue time is a **stale
//!   snapshot**. If an editor corrects a statement between enqueue and
//!   execution, a payload-carrying job would faithfully embed the text as it
//!   was, and the vector would silently disagree with the text a reader sees.
//!   Re-reading inside the worker makes the job read-your-writes against the
//!   graph at execution time, which is the only version that is actually
//!   correct.
//!
//! So the row holds an identifier, and the worker does the lookup. The queue
//! stores a *reference to work*, not a copy of its inputs.

use std::sync::Arc;

use apalis::prelude::{Data, WorkerContext};
use apalis_core::error::BoxDynError;
use auohp_core::embeddings::EmbedderHandle;
use neo4rs::{BoltMap, BoltString, BoltType, query};
use serde::{Deserialize, Serialize};

use crate::neo4j::Db;

/// How many (uid, vector) pairs go into one Cypher write.
///
/// Matches the batch size the inline implementation used. Large enough that
/// per-round-trip overhead is amortized, small enough that one failed write
/// does not discard an interview's worth of inference.
const EMBED_WRITE_BATCH: usize = 500;

/// Arguments for the embedding job.
///
/// `Serialize + Deserialize` is not decoration --- it is the trait bound that
/// makes this type legal as a task argument at all. apalis's `JsonCodec`
/// requires it in order to turn the value into the `job` BLOB column, and the
/// worker's `poll` requires it to turn that BLOB back into a Rust value. The
/// bound is what physically separates a durable queue from an in-process one:
/// a `tokio::mpsc` can move any `Send` value, a persistent queue can only move
/// values that survive a round trip through bytes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedInterview {
    /// The `Interview.uid` whose statements should be embedded.
    pub interview_uid: String,
}

impl EmbedInterview {
    /// The apalis queue this job is filed under.
    ///
    /// apalis partitions the `Jobs` table by a `job_type` column and every
    /// fetch query filters on it, so distinct queues can share one database
    /// file without seeing each other's work.
    ///
    /// The name lives here, on the job type, rather than as a free-standing
    /// constant --- and that placement is load-bearing. The enqueue side and
    /// the worker side each build their own `SqliteStorage` view, and both must
    /// name the same queue: `fetch_next.sql` filters `WHERE job_type = ?2`, so
    /// a mismatch means the worker polls for a `job_type` no row carries. Jobs
    /// would sit `Pending` forever with no error on either side --- nothing in
    /// the type system catches a disagreement between two string literals.
    ///
    /// Hanging the name off the job type makes that a correspondence rather
    /// than a convention: both sides write `EmbedInterview::QUEUE`, so they
    /// derive it from the same place and cannot drift apart.
    pub const QUEUE: &str = "auohp-embeddings";

    /// Construct a job for one interview.
    pub fn new(interview_uid: impl Into<String>) -> Self {
        Self {
            interview_uid: interview_uid.into(),
        }
    }
}

/// Everything the job body needs that is not part of its arguments.
///
/// This is the job-side mirror of what async-graphql resolvers get from
/// `ctx.data()`. apalis's equivalent is `WorkerBuilder::data`, and the
/// retrieval side is the [`Data`] extractor in the handler signature --- the
/// same `FromRequest`-style pattern axum uses for `State`, applied to a task
/// instead of an HTTP request.
///
/// Bundling both dependencies in one struct (rather than calling `.data()`
/// twice) keeps the handler signature to a single extractor and makes the
/// job's full dependency set legible in one place.
#[derive(Clone)]
pub struct EmbedDeps {
    pub db: Db,
    pub embedder: Arc<EmbedderHandle>,
}

/// Build a BoltMap from string-key / BoltType-value pairs.
fn bolt_map(pairs: Vec<(&str, BoltType)>) -> BoltType {
    let map: BoltMap = pairs
        .into_iter()
        .map(|(k, v)| (BoltString::from(k), v))
        .collect();
    BoltType::Map(map)
}

/// Read every statement belonging to an interview, in transcript order.
///
/// Returns `(uid, text)` pairs. Ordering is not strictly required --- each
/// vector is written back by uid --- but it keeps the batches stable, which
/// makes a partially-completed run easier to reason about.
async fn load_statements(db: &Db, interview_uid: &str) -> anyhow::Result<Vec<(String, String)>> {
    let mut stream = db
        .execute(
            query(
                r#"
                    MATCH (interview:Interview {uid: $interviewUid})
                          -[:HAS_TRANSCRIPT]->(:Transcript)
                          -[contains:CONTAINS]->(statement:Statement)
                    RETURN statement.uid AS uid, statement.text AS text
                    ORDER BY contains.startTime
                "#,
            )
            .param("interviewUid", interview_uid),
        )
        .await?;

    let mut out = Vec::new();
    while let Some(row) = stream.next().await? {
        // A Statement with no text is not an error worth failing the whole
        // interview over --- skip it and keep going.
        let (Ok(uid), Ok(text)) = (row.get::<String>("uid"), row.get::<String>("text")) else {
            continue;
        };
        out.push((uid, text));
    }

    Ok(out)
}

/// Write vectors back onto their Statement nodes.
///
/// `db.create.setNodeVectorProperty` is Neo4j's typed setter for vector
/// properties --- a plain `SET s.embedding = [...]` would store an ordinary
/// list and the vector index would not pick it up.
async fn write_embeddings(
    db: &Db,
    uids: &[String],
    vectors: &[Vec<f32>],
) -> anyhow::Result<()> {
    for (idx, chunk) in uids
        .iter()
        .zip(vectors.iter())
        .collect::<Vec<_>>()
        .chunks(EMBED_WRITE_BATCH)
        .enumerate()
    {
        tracing::debug!(batch = idx, size = chunk.len(), "writing embedding batch");

        let items: Vec<BoltType> = chunk
            .iter()
            .map(|(uid, vector)| {
                let vec_bolt: Vec<BoltType> =
                    vector.iter().map(|&v| BoltType::from(v as f64)).collect();
                bolt_map(vec![
                    ("uid", BoltType::from(uid.as_str())),
                    ("vector", BoltType::from(vec_bolt)),
                ])
            })
            .collect();

        db.run(query!(
            "
                UNWIND {items} AS item
                MATCH (s:Statement {{uid: item.uid}})
                CALL db.create.setNodeVectorProperty(s, 'embedding', item.vector)
            ",
            items = items,
        ))
        .await?;
    }

    Ok(())
}

/// The job body.
///
/// # Signature mechanics
///
/// This is a plain async fn, and apalis lifts it into a
/// `tower::Service<Task<EmbedInterview, SqliteContext, Ulid>>` via `task_fn`.
/// The lifting is driven by trait resolution over the parameter list: each
/// parameter after the first must implement `FromRequest`, which is how
/// `Data<EmbedDeps>` and `WorkerContext` get injected without appearing at the
/// call site. The first parameter is the decoded argument itself. It is the
/// same trick axum plays with handler extractors, and it means the function
/// stays an ordinary testable async fn --- nothing here is a macro.
///
/// # Error type
///
/// `BoxDynError` (`Box<dyn Error + Send + Sync>`) rather than a concrete error,
/// because the retry layer inspects errors dynamically: `RetryPolicy`
/// downcasts to check for `AbortError`, which is the signal for "this failure
/// is terminal, do not retry". Returning a boxed trait object is what makes
/// that downcast possible.
///
/// # Idempotency
///
/// Every effect here is `MATCH ... SET`-shaped, so a retry or an
/// orphan-recovery re-run converges to the same graph state. That property is
/// what licenses the at-least-once delivery the storage layer provides.
pub async fn embed_interview(
    args: EmbedInterview,
    deps: Data<EmbedDeps>,
    worker: WorkerContext,
) -> Result<(), BoxDynError> {
    let interview_uid = args.interview_uid;

    tracing::info!(
        interview_uid,
        worker = worker.name(),
        "embedding job started"
    );

    // Re-read at execution time. See the module docs for why this is not
    // merely a memory optimization.
    let statements = load_statements(&deps.db, &interview_uid).await?;

    if statements.is_empty() {
        // Not an error. An interview may legitimately have no statements, and
        // failing here would burn retries on a job that can never succeed.
        tracing::warn!(interview_uid, "no statements to embed");
        return Ok(());
    }

    // Split the pairs into parallel vectors. `unzip` consumes the Vec of
    // tuples and produces two owned Vecs in one pass --- no clone of the
    // strings, just a move of each half into its own allocation.
    let (uids, texts): (Vec<String>, Vec<String>) = statements.into_iter().unzip();

    tracing::info!(
        interview_uid,
        count = texts.len(),
        "submitting texts to embedder"
    );

    // `embed_background`, not `embed`. The distinction matters and is easy to
    // get wrong: `EmbedderHandle` is *not* a Mutex around a model. It is a
    // handle to a dedicated OS thread that already owns the ONNX session, and
    // it exposes two prioritized channels. `embed_background` goes on the
    // low-priority channel, which the worker thread services only when no
    // interactive search is waiting, and which it slices internally so a
    // search arriving mid-interview waits one sub-batch rather than the whole
    // job.
    //
    // For the same reason this call is `.await`ed directly and is *not*
    // wrapped in `spawn_blocking`. The blocking work happens on the embedder's
    // own thread; this future is just parked on a oneshot. Wrapping it would
    // occupy a tokio blocking-pool slot with a thread whose only job is to
    // wait.
    let vectors = deps.embedder.embed_background(texts).await?;

    tracing::info!(
        interview_uid,
        count = vectors.len(),
        dims = vectors.first().map(Vec::len).unwrap_or(0),
        "embedding complete, writing to Neo4j"
    );

    write_embeddings(&deps.db, &uids, &vectors).await?;

    tracing::info!(interview_uid, "embedding job finished");

    Ok(())
}
