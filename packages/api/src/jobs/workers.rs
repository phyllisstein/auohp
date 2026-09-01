//! Concrete workers.
//!
//! One real worker, [`EmbedInterviewWorker`], which moves the fire-and-forget
//! `tokio::spawn` currently buried in `seed_interview` onto the job system. It
//! is the honest motivating case: minutes of ONNX inference over a whole
//! interview, plus batched writes back to Neo4j.

use serde::{Deserialize, Serialize};

use crate::jobs::handle::JobId;
use crate::jobs::worker::{JobContext, Worker};

/// Arguments for [`EmbedInterviewWorker`].
///
/// `uids` and `texts` are parallel arrays, index-aligned: `texts[i]` is the
/// text of the statement with uid `uids[i]`. Parallel arrays rather than a
/// `Vec<(String, String)>` because the embedder's API takes and returns a flat
/// `Vec` in input order, so this shape avoids a zip-unzip round trip on every
/// call.
///
/// These derives are exactly the bounds `Worker<Args>` demands. `Serialize` is
/// used when the queue erases the args into JSON; `Deserialize` when the
/// handler recovers them. Today both happen in-process, microseconds apart ---
/// wasteful-looking, and deliberately so: it is the cost of keeping the durable
/// backend a drop-in. See the spike doc's note on the serialization tax.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedInterviewArgs {
    /// Uid of the interview, for logging and progress messages only.
    pub interview_uid: String,

    /// Statement uids, aligned with `texts`.
    pub uids: Vec<String>,

    /// Statement texts to embed, aligned with `uids`.
    pub texts: Vec<String>,
}

/// Embeds every statement of an interview and writes the vectors to Neo4j.
///
/// A unit struct: this worker holds no configuration, and everything it needs
/// arrives through `JobContext`. If it later grows a knob (batch size, say),
/// that becomes a field set at registration.
pub struct EmbedInterviewWorker;

/// How many statements are written to Neo4j per Cypher round trip. Matches the
/// constant the current `seed_interview` implementation uses.
const WRITE_BATCH: usize = 500;

impl Worker<EmbedInterviewArgs> for EmbedInterviewWorker {
    const NAME: &'static str = "embed_interview";

    async fn perform(
        &self,
        ctx: &JobContext,
        job_id: &JobId,
        args: EmbedInterviewArgs,
    ) -> anyhow::Result<()> {
        let EmbedInterviewArgs {
            interview_uid,
            uids,
            texts,
        } = args;

        if uids.len() != texts.len() {
            anyhow::bail!(
                "uids ({}) and texts ({}) must be the same length",
                uids.len(),
                texts.len()
            );
        }

        if uids.is_empty() {
            tracing::info!(job_id = %job_id, "nothing to embed");
            return Ok(());
        }

        tracing::info!(
            job_id = %job_id,
            interview_uid = %interview_uid,
            statements = uids.len(),
            "embedding interview statements"
        );

        ctx.report_progress(job_id, 0.0, "embedding");

        // Here is the part the brief cares about: CPU-bound work, and how it is
        // *not* handled here.
        //
        // The naive move would be `spawn_blocking(|| embedder.embed(texts))`.
        // That would be wrong in this codebase, and the reason is worth stating
        // precisely. `EmbedderHandle` is not an `Embedder` behind a lock --- it
        // is a handle to a dedicated OS thread that already owns the ONNX
        // session outright, with its own priority and background queues. The
        // blocking work has already been moved off the async runtime, once,
        // inside `auohp-core`. Wrapping this call in `spawn_blocking` would add
        // a second thread that does nothing but block on a channel waiting for
        // the first --- burning a blocking-pool slot to no purpose.
        //
        // So the rule for this job system is: a worker never calls
        // `spawn_blocking` on something that already owns its own thread. It
        // awaits the handle. `spawn_blocking` is the right tool for the *other*
        // pending job type, whisper.cpp transcription, which has no such
        // handle --- see the spike doc.
        //
        // `embed_background` rather than `embed` is also deliberate: it marks
        // this as bulk work, so the embedder services interactive search
        // requests between sub-batches instead of making them wait out a whole
        // interview.
        let vectors = ctx
            .embedder()?
            .embed_background(texts)
            .await
            .map_err(|e| anyhow::anyhow!("embedding failed: {e}"))?;

        if vectors.len() != uids.len() {
            anyhow::bail!(
                "embedder returned {} vectors for {} statements",
                vectors.len(),
                uids.len()
            );
        }

        tracing::info!(
            job_id = %job_id,
            count = vectors.len(),
            dims = vectors.first().map_or(0, Vec::len),
            "embedding complete, writing to neo4j"
        );

        ctx.report_progress(job_id, 0.5, "writing vectors");

        // Write back in batches. Progress spans 0.5..=1.0 so the bar reflects
        // the whole job, not just this phase.
        let pairs: Vec<(&String, &Vec<f32>)> = uids.iter().zip(vectors.iter()).collect();
        let batch_count = pairs.len().div_ceil(WRITE_BATCH);

        for (index, chunk) in pairs.chunks(WRITE_BATCH).enumerate() {
            let items: Vec<neo4rs::BoltType> = chunk
                .iter()
                .map(|(uid, vector)| {
                    let vector: Vec<neo4rs::BoltType> = vector
                        .iter()
                        .map(|&v| neo4rs::BoltType::from(f64::from(v)))
                        .collect();

                    let map: neo4rs::BoltMap = [
                        (
                            neo4rs::BoltString::from("uid"),
                            neo4rs::BoltType::from(uid.as_str()),
                        ),
                        (
                            neo4rs::BoltString::from("vector"),
                            neo4rs::BoltType::from(vector),
                        ),
                    ]
                    .into_iter()
                    .collect();

                    neo4rs::BoltType::Map(map)
                })
                .collect();

            ctx.db
                .run(
                    neo4rs::query(
                        "UNWIND $items AS item
                         MATCH (s:Statement {uid: item.uid})
                         CALL db.create.setNodeVectorProperty(s, 'embedding', item.vector)",
                    )
                    .param("items", items),
                )
                .await
                .map_err(|e| anyhow::anyhow!("failed to write embedding batch {index}: {e}"))?;

            // `index + 1` because progress reports work *finished*, not started.
            #[allow(clippy::cast_precision_loss)]
            let fraction = 0.5 + 0.5 * ((index + 1) as f32 / batch_count as f32);

            ctx.report_progress(
                job_id,
                fraction,
                &format!("wrote batch {}/{batch_count}", index + 1),
            );
        }

        tracing::info!(job_id = %job_id, "embeddings written successfully");

        Ok(())
    }
}
