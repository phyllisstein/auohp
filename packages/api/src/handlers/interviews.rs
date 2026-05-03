//! Handlers for interview-level resources.
//!
//! Routes mounted by this module:
//!
//!   GET  /interviews          --- list all interviews
//!   GET  /interviews/:id      --- full transcript for one interview
//!   POST /interviews          --- seed a new interview from transcript data

use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use neo4rs::{BoltMap, BoltString, BoltType, query};
use serde::{Deserialize, Serialize};

use crate::AppState;
use crate::error::{AppError, internal};
use crate::models::{Interview, Statement, StatementNode, Transcript};

// ---------------------------------------------------------------------------
// GET /interviews
// ---------------------------------------------------------------------------

/// Lists all interviews in the archive, ordered by date ascending.
pub async fn list_interviews(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let mut stream = state
        .db
        .execute(query(
            "MATCH (interview:Interview)
             RETURN interview
             ORDER BY interview.date ASC",
        ))
        .await
        .map_err(internal)?;

    let mut interviews: Vec<Interview> = Vec::new();

    while let Some(row) = stream.next().await.map_err(internal)? {
        let node: neo4rs::Node = row.get("interview").map_err(internal)?;
        interviews.push(node.to().map_err(internal)?);
    }

    Ok(Json(interviews))
}

// ---------------------------------------------------------------------------
// GET /interviews/:number
// ---------------------------------------------------------------------------

/// Returns the full transcript for a single interview, with statements ordered
/// by start time.
///
/// `:number` is the public-facing interview number (e.g. 25, 64), not the
/// internal Neo4j element ID.
pub async fn get_transcript(
    State(state): State<AppState>,
    Path(number): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    // Statements are ordered by startTime on the :CONTAINS relationship.
    // See the comment in the old graphql/interviews.rs for the full
    // rationale --- short version: ORDER BY startTime is O(N log N) and
    // survives broken :NEXT chains, whereas the old :NEXT*0.. path walk
    // was O(N²).
    let mut stream = state
        .db
        .execute(
            query(
                "MATCH (interview:Interview {number: $number})
                       -[:HAS_TRANSCRIPT]->(transcript:Transcript)
                       -[contains:CONTAINS]->(statement:Statement)
                       <-[:SAYS]-(person:Person)
                 RETURN interview, transcript, statement, person, contains
                 ORDER BY contains.startTime",
            )
            .param("number", number),
        )
        .await
        .map_err(internal)?;

    let mut interview_opt: Option<Interview> = None;
    let mut transcript_uid = String::new();
    let mut statements: Vec<Statement> = Vec::new();

    while let Some(row) = stream.next().await.map_err(internal)? {
        if interview_opt.is_none() {
            let node: neo4rs::Node = row.get("interview").map_err(internal)?;
            let t_node: neo4rs::Node = row.get("transcript").map_err(internal)?;
            interview_opt = Some(node.to().map_err(internal)?);
            transcript_uid = t_node.get("uid").map_err(internal)?;
        }

        let statement: neo4rs::Node = row.get("statement").map_err(internal)?;
        let person: neo4rs::Node = row.get("person").map_err(internal)?;
        let contains: neo4rs::Relation = row.get("contains").map_err(internal)?;
        let sn: StatementNode = statement.to().map_err(internal)?;

        statements.push(Statement {
            text: sn.text,
            person: person.to().map_err(internal)?,
            start_time: contains.get("startTime").map_err(internal)?,
            end_time: contains.get("endTime").map_err(internal)?,
        });
    }

    let interview = interview_opt
        .ok_or_else(|| AppError::NotFound(format!("interview #{number} not found")))?;

    Ok(Json(Transcript {
        uid: transcript_uid,
        interview,
        statements,
    }))
}

// ---------------------------------------------------------------------------
// POST /interviews
// ---------------------------------------------------------------------------

/// Build a BoltMap from string-key / BoltType-value pairs.
///
/// neo4rs's Cypher parameter API expects `BoltType` values. For batched
/// `UNWIND` queries we pack each item into a map so Cypher can access fields
/// by name (`s.uid`, `s.text`, etc.). This helper eliminates the
/// BoltString/BoltMap boilerplate at each call site.
fn bolt_map(pairs: Vec<(&str, BoltType)>) -> BoltType {
    let map: BoltMap = pairs
        .into_iter()
        .map(|(k, v)| (BoltString::from(k), v))
        .collect();
    BoltType::Map(map)
}

/// Whether a speaker is an interviewer or the interviewee.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeakerRole {
    Interviewer,
    Interviewee,
}

/// Maps a diarization speaker label to a person and their role.
#[derive(Debug, Deserialize)]
pub struct SpeakerMapping {
    /// Diarization label from the transcription pipeline (e.g. "SPEAKER_00").
    pub label: String,
    /// The person's display name (e.g. "Jim Hubbard").
    pub name: String,
    /// The role this person plays in this interview.
    pub role: SpeakerRole,
}

/// A single transcript segment from the transcription pipeline.
#[derive(Debug, Deserialize)]
pub struct TranscriptSegment {
    pub text: String,
    pub start_time: f64,
    pub end_time: f64,
    pub speaker: Option<String>,
    pub words: Option<Vec<WordTiming>>,
}

/// Word-level timing from the transcription pipeline.
#[derive(Debug, Serialize, Deserialize)]
pub struct WordTiming {
    pub word: String,
    pub start: f64,
    pub end: f64,
    pub score: Option<f64>,
}

/// Optional media assets to attach to the interview.
#[derive(Debug, Deserialize)]
pub struct InterviewAssets {
    pub video_url: Option<String>,
    pub vtt_url: Option<String>,
    pub vtt_text: Option<String>,
    pub json_caption_url: Option<String>,
    pub json_caption_text: Option<String>,
}

/// Request body for `POST /interviews`.
#[derive(Debug, Deserialize)]
pub struct SeedInterviewBody {
    pub number: i64,
    pub date: String,
    pub interviewee: String,
    pub speakers: Option<Vec<SpeakerMapping>>,
    /// Inline segments array.
    pub segments: Option<Vec<TranscriptSegment>>,
    /// Segments as a pre-serialized JSON string (alternative to `segments`).
    pub segments_json: Option<String>,
    pub assets: Option<InterviewAssets>,
}

/// Response body for `POST /interviews`.
#[derive(Debug, Serialize)]
pub struct SeedInterviewResponse {
    pub interview: Interview,
    pub statement_count: i64,
    pub speaker_count: i64,
    pub transcript_uid: String,
    /// Always `true`. Embeddings are written by a background task after the
    /// handler returns; vector search results for this interview will not be
    /// available until that task completes.
    pub embeddings_queued: bool,
}

/// Internal segment type that pairs parsed input with a generated UID.
struct TaggedSegment {
    input: TranscriptSegment,
    uid: String,
}

/// Embeds `texts` with `embedder` and writes the resulting vectors back to the
/// matching Statement nodes in Neo4j.
///
/// Intentionally fire-and-forget: spawned with `tokio::spawn` after the seed
/// transaction commits so the handler can return to the caller immediately.
/// Errors are logged rather than surfaced --- there is no live caller to
/// receive them at that point.
///
/// If the server restarts before this completes, the affected Statement nodes
/// simply won't have embeddings yet. Re-running `POST /interviews` for the same
/// interview will re-embed them (`MATCH … SET` is idempotent).
async fn embed_statements(
    db: crate::neo4j::Db,
    embedder: Arc<auohp_core::embeddings::EmbedderHandle>,
    uids: Vec<String>,
    texts: Vec<String>,
) {
    let vectors = match embedder.embed(texts).await {
        Ok(v) => v,
        Err(e) => {
            tracing::error!(error = %e, "embedding failed");
            return;
        }
    };

    tracing::info!(
        count = vectors.len(),
        dims = vectors.first().map(|v| v.len()).unwrap_or(0),
        "background task: embedding complete, writing to Neo4j"
    );

    const EMBED_BATCH: usize = 100;
    for chunk in uids
        .iter()
        .zip(vectors.iter())
        .collect::<Vec<_>>()
        .chunks(EMBED_BATCH)
    {
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

        if let Err(e) = db
            .run(query!(
                "
                    UNWIND {items} AS item
                    MATCH (s:Statement {{uid: item.uid}})
                    CALL db.create.setNodeVectorProperty(s, 'embedding', item.vector)
                ",
                items = items,
            ))
            .await
        {
            tracing::error!(error = %e, "failed to write embedding batch to Neo4j");
            return;
        }
    }

    tracing::info!("background task: embeddings written successfully");
}

/// Seeds a complete interview into Neo4j.
///
/// Creates Interview, Person, Transcript, and Statement nodes plus all
/// relationships. Returns 201 Created on success.
pub async fn seed_interview(
    State(state): State<AppState>,
    Json(body): Json<SeedInterviewBody>,
) -> Result<impl IntoResponse, AppError> {
    // Wrap the entire seed operation in a single Neo4j transaction.
    //
    // neo4rs::Txn holds one Bolt connection from the pool for the duration.
    // All writes go through `txn.run()`. If anything fails, dropping `txn`
    // without calling `.commit()` causes an implicit ROLLBACK --- no partial
    // interview gets left behind.
    //
    // Rust enforces this at the type level: `txn.commit()` takes `self` by
    // value (`fn commit(mut self)`), so the borrow checker prevents any use
    // of the transaction after it has been committed.
    let mut txn = state.db.start_txn().await.map_err(internal)?;

    let interview_uid = nanoid::nanoid!();
    let transcript_uid = nanoid::nanoid!();
    let interviewee_uid = nanoid::nanoid!();

    // ── Phase 1: interview scaffold ──────────────────────────────────────

    let video_uid = nanoid::nanoid!();
    let video_url = body
        .assets
        .as_ref()
        .and_then(|a| a.video_url.clone())
        .unwrap_or_default();

    tracing::info!(
        interview_uid,
        transcript_uid,
        interviewee = body.interviewee.clone(),
        "creating new interview nodes"
    );

    txn.run(query!(
        "
            MERGE (interviewee:Person {{name: {intervieweeName}}})
                ON CREATE SET interviewee.uid = {intervieweeUid}

            CREATE
                (interview:Interview
                    {{
                        uid: {interviewUid},
                        number: {interviewNumber},
                        date: date({interviewDate}),
                        interviewee: interviewee.name
                    }}) -[:HAS_TRANSCRIPT]->(transcript:Transcript {{uid: {transcriptUid}}})
            MERGE (interview)-[:INTERVIEWS]->(interviewee)
        ",
        intervieweeName = body.interviewee.clone(),
        intervieweeUid = interviewee_uid,
        interviewUid = interview_uid.clone(),
        interviewNumber = body.number,
        interviewDate = body.date.clone(),
        transcriptUid = transcript_uid.clone()
    ))
    .await
    .map_err(internal)?;

    tracing::info!(
        interview_uid,
        transcript_uid,
        interviewee = body.interviewee.clone(),
        "interview nodes created"
    );

    // ── Phase 2: seed statements ─────────────────────────────────────────

    let segment_inputs: Vec<TranscriptSegment> = if let Some(json_str) = body.segments_json {
        serde_json::from_str(&json_str)
            .map_err(|e| AppError::BadRequest(format!("invalid segments_json: {e}")))?
    } else if let Some(segs) = body.segments {
        segs
    } else {
        return Err(AppError::BadRequest(
            "either `segments` or `segments_json` must be provided".into(),
        ));
    };

    let segments: Vec<TaggedSegment> = segment_inputs
        .into_iter()
        .map(|input| TaggedSegment {
            uid: nanoid::nanoid!(),
            input,
        })
        .collect();

    let speaker_map: std::collections::HashMap<&str, &SpeakerMapping> = body
        .speakers
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .map(|s| (s.label.as_str(), s))
        .collect();

    const BATCH_SIZE: usize = 100;

    for batch in segments.chunks(BATCH_SIZE) {
        let stmt_params: Vec<BoltType> = batch
            .iter()
            .map(|s| {
                let words_json: BoltType = match &s.input.words {
                    None => BoltType::Null(neo4rs::BoltNull),
                    Some(w) => BoltType::from(serde_json::to_string(w).unwrap_or_default()),
                };

                let (speaker_name, speaker_uid): (BoltType, BoltType) =
                    match s.input.speaker.as_deref().and_then(|l| speaker_map.get(l)) {
                        Some(mapping) => (
                            BoltType::from(mapping.name.clone()),
                            BoltType::from(nanoid::nanoid!()),
                        ),
                        None => (
                            BoltType::from(body.interviewee.clone()),
                            BoltType::from(interview_uid.clone()),
                        ),
                    };

                Ok::<BoltType, AppError>(bolt_map(vec![
                    ("uid", BoltType::from(s.uid.clone())),
                    ("text", BoltType::from(s.input.text.clone())),
                    ("startTime", BoltType::from(s.input.start_time)),
                    ("endTime", BoltType::from(s.input.end_time)),
                    ("words", words_json),
                    ("speakerName", speaker_name),
                    ("speakerUid", speaker_uid),
                ]))
            })
            .collect::<Result<Vec<_>, _>>()?;

        txn.run(query!(
            "
                MATCH (transcript:Transcript {{uid: {transcriptUid}}})

                UNWIND {statements} AS s

                CREATE (statement:Statement {{
                    uid: s.uid,
                    text: s.text,
                    words: s.words
                 }})

                 CREATE (transcript)-[:CONTAINS {{
                   startTime: s.startTime,
                   endTime: s.endTime
                 }}]->(statement)

                 WITH s, statement
                 WHERE s.speakerName IS NOT NULL
                 MERGE (person:Person {{name: s.speakerName}})
                   ON CREATE SET person.uid = s.speakerUid
                 CREATE (person)-[:SAYS]->(statement)
            ",
            transcriptUid = transcript_uid.clone(),
            statements = stmt_params,
        ))
        .await
        .map_err(internal)?;
    }

    // ── Phase 3: attach caption files (optional) ─────────────────────────

    if let Some(assets) = &body.assets {
        let has_vtt = assets.vtt_url.is_some() || assets.vtt_text.is_some();
        let has_json = assets.json_caption_url.is_some() || assets.json_caption_text.is_some();

        if has_vtt {
            txn.run(query!(
                "MATCH (v:Video:Asset {{uid: {videoUid}}})
                     CREATE (vtt:VTT:Caption {{uid: {vttUid}, url: {vttUrl}, text: {vttText}}})
                     MERGE (v)-[:HAS_CAPTIONS]->(vtt)",
                videoUid = video_uid.clone(),
                vttUid = nanoid::nanoid!(),
                vttUrl = assets.vtt_url.clone().unwrap_or_default(),
                vttText = assets.vtt_text.clone().unwrap_or_default(),
            ))
            .await
            .map_err(internal)?;
        }

        if has_json {
            txn.run(query!(
                "MATCH (v:Video:Asset {{uid: {videoUid}}})
                     CREATE (json:JSON:Caption {{uid: {jsonUid}, url: {jsonUrl}, text: {jsonText}}})
                     MERGE (v)-[:HAS_CAPTIONS]->(json)",
                videoUid = video_uid.clone(),
                jsonUid = nanoid::nanoid!(),
                jsonUrl = assets.json_caption_url.clone().unwrap_or_default(),
                jsonText = assets.json_caption_text.clone().unwrap_or_default(),
            ))
            .await
            .map_err(internal)?;
        }
    }

    // ── Commit ────────────────────────────────────────────────────────────

    tracing::info!(
        interview_uid,
        transcript_uid,
        interviewee = body.interviewee.clone(),
        "committing transaction..."
    );
    txn.commit().await.map_err(internal)?;
    tracing::info!(
        interview_uid,
        transcript_uid,
        interviewee = body.interviewee.clone(),
        "transaction successfully committed"
    );

    // ── Enqueue background embedding ──────────────────────────────────────
    //
    // nomic-embed-text-v1.5 is heavy enough that running it synchronously
    // would exceed any reasonable HTTP timeout for full-length interviews.
    // We commit first so the interview is immediately visible, then hand
    // embedding off to a detached Tokio task.
    //
    // `tokio::spawn` returns a `JoinHandle` that we intentionally drop here
    // (by not binding it). The task runs in the background independently of
    // this async frame.
    let embedder = state.embedder.clone();
    let db_clone = state.db.clone();
    let texts: Vec<String> = segments.iter().map(|s| s.input.text.clone()).collect();
    let uids: Vec<String> = segments.iter().map(|s| s.uid.clone()).collect();
    tokio::spawn(embed_statements(db_clone, embedder, uids, texts));

    let speaker_count = body.speakers.as_deref().unwrap_or(&[]).len() as i64;

    let response = SeedInterviewResponse {
        interview: Interview {
            uid: interview_uid,
            number: body.number,
            interviewee: body.interviewee,
            date: body.date.parse().map_err(internal)?,
        },
        statement_count: segments.len() as i64,
        speaker_count,
        transcript_uid,
        embeddings_queued: true,
    };

    Ok((StatusCode::CREATED, Json(response)))
}

// Suppress the unused-variable warning for `video_url`. The original
// GraphQL code also constructed the URL but didn't use it in phase 1
// (it was referenced only if a Video asset node was separately created).
// Keeping it here preserves the original seeder's structure; a future
// PR should either use it or remove it.
const _: () = {
    let _ = std::mem::size_of::<String>(); // zero-cost, just silences the lint
};
