use std::sync::Arc;

use async_graphql::{Context, Enum, InputObject, SimpleObject};
use neo4rs::{BoltMap, BoltString, BoltType, query};
use serde::{Deserialize, Serialize};

use crate::graphql::queries::interviews::Interview;
use crate::jobs::workers::{EmbedInterviewArgs, EmbedInterviewWorker};
use crate::jobs::Queue;
use crate::neo4j::Db;
use crate::uid;

/// Build a BoltMap from string-key / BoltType-value pairs.
fn bolt_map(pairs: Vec<(&str, BoltType)>) -> BoltType {
    let map: BoltMap = pairs
        .into_iter()
        .map(|(k, v)| (BoltString::from(k), v))
        .collect();
    BoltType::Map(map)
}

// ---------------------------------------------------------------------------
// Input types
// ---------------------------------------------------------------------------

/// Top-level input for the `seedInterview` mutation.
#[derive(InputObject)]
pub struct SeedInterviewInput {
    /// Interview number in the AUOHP archive (e.g. 25, 64, 82).
    pub number: i64,
    /// ISO 8601 date string, e.g. "2003-05-05".
    pub date: String,
    /// Display name for the interviewee (e.g. "Lei Chou").
    pub interviewee: String,
    /// Maps diarization labels to person names and roles.
    /// Optional --- omit when speaker labels are not yet assigned.
    pub speakers: Option<Vec<SpeakerMappingInput>>,
    /// Raw transcript segments from the transcription pipeline.
    /// Either `segments_json` or `segments` must be specified.
    pub segments: Option<Vec<TranscriptSegmentInput>>,
    /// JSON string of transcript segments.
    /// Either `segments_json` or `segments` must be specified.
    pub segments_json: Option<String>,
    /// Optional asset URLs associated with the interview.
    pub assets: Option<InterviewAssetsInput>,
}

/// Maps a diarization speaker label to a person and their role.
#[derive(InputObject)]
pub struct SpeakerMappingInput {
    /// Diarization label from the transcription pipeline (e.g. "SPEAKER_00").
    pub label: String,
    /// The person's display name (e.g. "Jim Hubbard").
    pub name: String,
    /// The role this person plays in this interview.
    pub role: SpeakerRole,
}

/// Whether a speaker is an interviewer or the interviewee.
#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum SpeakerRole {
    Interviewer,
    Interviewee,
}

/// A single transcript segment from the transcription pipeline.
#[derive(InputObject, Deserialize)]
pub struct TranscriptSegmentInput {
    /// Transcribed text for this segment.
    pub text: String,
    /// Start time in seconds from the beginning of the recording.
    pub start_time: f64,
    /// End time in seconds.
    pub end_time: f64,
    /// Speaker label (e.g. "SPEAKER_00"). Optional --- omit when not yet assigned.
    pub speaker: Option<String>,
    /// Per-word timing data. Optional---some segments may lack word alignment.
    pub words: Option<Vec<WordTimingInput>>,
}

struct TranscriptSegment {
    input: TranscriptSegmentInput,
    uid: String,
}

/// Word-level timing from the transcription pipeline.
#[derive(InputObject, Serialize, Deserialize)]
pub struct WordTimingInput {
    pub word: String,
    pub start: f64,
    pub end: f64,
    /// Confidence score (0.0–1.0). Optional.
    pub p: Option<f64>,
}

/// Optional media assets to attach to the interview.
#[derive(InputObject)]
pub struct InterviewAssetsInput {
    pub video_url: Option<String>,
    pub vtt_url: Option<String>,
    pub vtt_text: Option<String>,
    pub json_caption_url: Option<String>,
    pub json_caption_text: Option<String>,
}

// ---------------------------------------------------------------------------
// Payload (return type)
// ---------------------------------------------------------------------------

/// Returned by `seedInterview` to confirm what was created.
#[derive(SimpleObject)]
pub struct SeedInterviewPayload {
    pub interview: Interview,
    pub statement_count: i64,
    pub speaker_count: i64,
    pub transcript_uid: String,
    /// Always `true`. Embeddings are written by a background task after the
    /// mutation returns; vector search results for this interview will not be
    /// available until that task completes.
    pub embeddings_queued: bool,

    /// Id of the enqueued embedding job, for polling `query { job(id:) }`.
    ///
    /// `None` when embedding was skipped via `SKIP_SEED_EMBEDDING`. This field
    /// is the whole visible benefit of moving off the bare `tokio::spawn`: the
    /// old code handed the caller no way to learn whether the work it promised
    /// ever happened.
    pub embedding_job_id: Option<String>,
}

// ---------------------------------------------------------------------------
// Resolver
// ---------------------------------------------------------------------------

pub async fn seed_interview(
    ctx: &Context<'_>,
    input: SeedInterviewInput,
) -> async_graphql::Result<SeedInterviewPayload> {
    let db = ctx.data::<Db>()?;

    // Wrap the entire seed operation in a single Neo4j transaction.
    //
    // neo4rs::Txn holds one Bolt connection from the pool for the duration
    // of the transaction. All writes go through `txn.run()` instead of
    // `db.run()`, so they share a single server-side transaction context.
    // If anything fails, dropping `txn` without calling `.commit()` causes
    // an implicit ROLLBACK---no partial interview gets left behind.
    //
    // Rust enforces this at the type level: `txn.commit()` takes ownership
    // of `self` (it's `fn commit(mut self)`), so the borrow checker won't
    // let you accidentally use the transaction after committing.
    let mut txn = db.start_txn().await?;

    let interview_uid = uid::generate();
    let transcript_uid = uid::generate();
    let interviewee_uid = uid::generate();

    txn.run(
        query(
            r#"
                MATCH (p)<-[:INTERVIEWS]-(i:Interview {number: $interviewNumber})-[:HAS_TRANSCRIPT]->(t)-[:CONTAINS]->(s)
                MATCH (i)-[:HAS_ASSET]->(a)
                DETACH DELETE a, i, s, t, p
            "#
        ).param("interviewNumber", input.number),
    ).await?;

    // ── Phase 1: interview scaffold ──────────────────────────────────────

    tracing::info!(
        interview_uid,
        transcript_uid,
        interviewee = input.interviewee.clone(),
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
        intervieweeName = input.interviewee.clone(),
        intervieweeUid = interviewee_uid,
        interviewUid = interview_uid.clone(),
        interviewNumber = input.number,
        interviewDate = input.date.clone(),
        transcriptUid = transcript_uid.clone(),
    ))
    .await?;

    let video_uid = uid::generate();
    let video_url = input.assets.as_ref().and_then(|a| a.video_url.clone());
    tracing::debug!(
        video_uid = video_uid.clone(),
        video_url = video_url.clone(),
        interviewee = input.interviewee.clone(),
        interview_number = input.number,
        "seeding with video"
    );

    if video_url.is_some() {
        txn.run(query!(
            r#"
                MATCH (i:Interview {{uid: {interviewUid}}})
                CREATE (i)-[:HAS_ASSET]->(video:Video:Asset {{uri: {uri}, uid: {videoUid}}})
                RETURN video
            "#,
            uri = video_url.clone(),
            videoUid = video_uid.clone(),
            interviewUid = interview_uid.clone(),
        ))
        .await?
    }

    tracing::info!(
        interview_uid,
        transcript_uid,
        interviewee = input.interviewee.clone(),
        "interview nodes created"
    );

    // ── Phase 2: seed statements ───────────────────────────────────────

    // Batch statements in groups of 100
    const BATCH_SIZE: usize = 500;

    let segment_inputs: Vec<TranscriptSegmentInput> = if let Some(segment_json) =
        input.segments_json
    {
        serde_json::from_str(&segment_json).map_err(|e| async_graphql::Error::new(e.to_string()))
    } else if let Some(segment_gql) = input.segments {
        Ok(segment_gql)
    } else {
        Err(async_graphql::Error::new(
            "Either segments or segmentsJson must be provided",
        ))
    }?;

    let segments: Vec<TranscriptSegment> = segment_inputs
        .into_iter()
        .map(|i| TranscriptSegment {
            input: i,
            uid: uid::generate(),
        })
        .collect();

    let speaker_map: std::collections::HashMap<&str, &SpeakerMappingInput> = input
        .speakers
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .map(|s| (s.label.as_str(), s))
        .collect();

    for batch in segments.chunks(BATCH_SIZE) {
        let stmt_params: Vec<BoltType> = batch
            .iter()
            .map(|s| {
                let words_json: BoltType = if s.input.words.is_none() {
                    BoltType::Null(neo4rs::BoltNull)
                } else {
                    BoltType::from(serde_json::to_string(&s.input.words).unwrap_or_default())
                };

                let segment = &s.input;

                // Resolve the speaker label to a name if a mapping exists. In
                // the absence of a specific mapping, assume the speaker is the
                // interviewee.
                let (speaker_name, speaker_uid): (BoltType, BoltType) =
                    match segment.speaker.as_deref().and_then(|l| speaker_map.get(l)) {
                        Some(mapping) => (
                            BoltType::from(mapping.name.clone()),
                            BoltType::from(uid::generate()),
                        ),
                        None => (
                            BoltType::from(input.interviewee.clone()),
                            BoltType::from(interview_uid.clone()),
                        ),
                    };

                Ok::<BoltType, async_graphql::Error>(bolt_map(vec![
                    ("uid", BoltType::from(s.uid.clone())),
                    ("text", BoltType::from(segment.text.clone())),
                    ("startTime", BoltType::from(segment.start_time)),
                    ("endTime", BoltType::from(segment.end_time)),
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
        .await?;
    }

    // ── Phase 3: attach caption files (optional) ────────────────────────

    if let Some(assets) = &input.assets {
        let has_vtt = assets.vtt_url.is_some() || assets.vtt_text.is_some();
        let has_json = assets.json_caption_url.is_some() || assets.json_caption_text.is_some();

        if has_vtt {
            txn.run(query!(
                "MATCH (v:Video:Asset {{uid: {videoUid}}})
                     CREATE (vtt:VTT:Caption {{uid: {vttUid}, url: {vttUrl}, text: {vttText}}})
                     MERGE (v)-[:HAS_CAPTIONS]->(vtt)",
                videoUid = video_uid.clone(),
                vttUid = uid::generate(),
                vttUrl = assets.vtt_url.clone().unwrap_or_default(),
                vttText = assets.vtt_text.clone().unwrap_or_default(),
            ))
            .await?;
        }

        if has_json {
            txn.run(query!(
                "MATCH (v:Video:Asset {{uid: {videoUid}}})
                     CREATE (json:JSON:Caption {{uid: {jsonUid}, url: {jsonUrl}, text: {jsonText}}})
                     MERGE (v)-[:HAS_CAPTIONS]->(json)",
                videoUid = video_uid.clone(),
                jsonUid = uid::generate(),
                jsonUrl = assets.json_caption_url.clone().unwrap_or_default(),
                jsonText = assets.json_caption_text.clone().unwrap_or_default(),
            ))
            .await?;
        }
    }

    // ── Commit ────────────────────────────────────────────────────────────
    //
    // All writes succeeded---commit the transaction. This is the only
    // point where data becomes visible to other connections. If we never
    // reach this line (early return, ?, or panic), the Txn is dropped
    // without committing and Neo4j rolls back automatically.
    tracing::info!(
        interview_uid,
        transcript_uid,
        interviewee = input.interviewee.clone(),
        "committing transaction..."
    );
    txn.commit().await?;
    tracing::info!(
        interview_uid,
        transcript_uid,
        interviewee = input.interviewee.clone(),
        "transaction successfully committed"
    );

    // ── Enqueue background embedding ──────────────────────────────────────
    //
    // nomic-embed-text-v1.5 is heavy enough that running it synchronously
    // would exceed any reasonable HTTP timeout for full-length interviews.
    // We commit first so the interview is immediately visible, then hand
    // embedding off to the background job system.
    //
    // This used to be a bare `tokio::spawn` whose `JoinHandle` was dropped on
    // the floor. That worked, but bought nothing back: no id, no status, no
    // backpressure, and a panic inside the task vanished silently. Going
    // through the queue keeps the same non-blocking behaviour and adds all
    // four. The failure mode it does *not* fix is durability --- a restart
    // still loses queued work, exactly as before.
    let skip_embedding = match std::env::var("SKIP_SEED_EMBEDDING") {
        Ok(val) => !val.is_empty(),
        Err(_) => false,
    };

    let embedding_job_id = if skip_embedding {
        tracing::info!(
            interview_uid,
            transcript_uid,
            interviewee = input.interviewee.clone(),
            "skipping embeddings: SKIP_SEED_EMBEDDING envvar is set"
        );

        None
    } else {
        let queue = ctx.data::<Queue>()?;

        let texts: Vec<String> = segments
            .iter()
            .map(|s| &s.input)
            .map(|i| i.text.clone())
            .collect();
        let uids: Vec<String> = segments.iter().map(|s| s.uid.clone()).collect();

        tracing::info!(
            statement_count = texts.len(),
            interview_uid,
            transcript_uid,
            interviewee = input.interviewee.clone(),
            "populating statement embeddings"
        );

        // The turbofish names the worker; `Args` is inferred from the value.
        let handle = queue
            .enqueue::<_, EmbedInterviewWorker>(EmbedInterviewArgs {
                interview_uid: interview_uid.clone(),
                uids,
                texts,
            })
            .await?;

        Some(handle.id.to_string())
    };

    // ── Build response ──────────────────────────────────────────────────

    let speaker_count = input.speakers.as_deref().unwrap_or(&[]).len() as i64;

    Ok(SeedInterviewPayload {
        interview: Interview {
            uid: interview_uid,
            number: input.number,
            date: input.date.parse()?,
        },
        statement_count: segments.len() as i64,
        speaker_count,
        transcript_uid,
        embeddings_queued: !skip_embedding,
        embedding_job_id,
    })
}
