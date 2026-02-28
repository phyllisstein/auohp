use async_graphql::{Context, Enum, InputObject, SimpleObject};
use neo4rs::{query, BoltMap, BoltString, BoltType};
use serde::Serialize;

use crate::neo4j::Db;
use super::super::interviews::Interview;

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
    pub speakers: Vec<SpeakerMappingInput>,
    /// Raw transcript segments from the transcription pipeline.
    /// The server groups consecutive same-speaker segments into statements.
    pub segments: Vec<TranscriptSegmentInput>,
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
#[derive(InputObject)]
pub struct TranscriptSegmentInput {
    /// Transcribed text for this segment.
    pub text: String,
    /// Start time in seconds from the beginning of the recording.
    pub start_time: f64,
    /// End time in seconds.
    pub end_time: f64,
    /// Diarization speaker label (must match one of the labels in speakers).
    pub speaker: String,
    /// Per-word timing data. Optional — some segments may lack word alignment.
    pub words: Option<Vec<WordTimingInput>>,
}

/// Word-level timing from the transcription pipeline.
#[derive(InputObject, Serialize)]
pub struct WordTimingInput {
    pub word: String,
    pub start: f64,
    pub end: f64,
    /// Confidence score (0.0–1.0). Optional.
    pub score: Option<f64>,
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
}

// ---------------------------------------------------------------------------
// Resolver
// ---------------------------------------------------------------------------

fn gql_err(e: impl std::fmt::Display) -> async_graphql::Error {
    async_graphql::Error::new(e.to_string())
}

/// A merged statement ready for seeding — consecutive same-speaker segments
/// grouped into a single statement.
struct MergedStatement {
    speaker_name: String,
    text: String,
    start_time: f64,
    end_time: f64,
    words: Vec<WordTimingInput>,
}

/// Groups consecutive same-speaker segments into statements.
fn group_segments(
    input: &SeedInterviewInput,
) -> async_graphql::Result<Vec<MergedStatement>> {
    let speaker_map: std::collections::HashMap<&str, &SpeakerMappingInput> = input
        .speakers
        .iter()
        .map(|s| (s.label.as_str(), s))
        .collect();

    let mut statements: Vec<MergedStatement> = Vec::new();

    for segment in &input.segments {
        let mapping = speaker_map.get(segment.speaker.as_str()).ok_or_else(|| {
            async_graphql::Error::new(format!(
                "speaker label {:?} not found in speaker mappings",
                segment.speaker
            ))
        })?;

        let should_merge = statements
            .last()
            .map(|prev| prev.speaker_name == mapping.name)
            .unwrap_or(false);

        if should_merge {
            let current = statements.last_mut().unwrap();
            current.text.push(' ');
            current.text.push_str(&segment.text);
            current.end_time = segment.end_time;
            if let Some(words) = &segment.words {
                current.words.extend(words.iter().map(|w| WordTimingInput {
                    word: w.word.clone(),
                    start: w.start,
                    end: w.end,
                    score: w.score,
                }));
            }
        } else {
            statements.push(MergedStatement {
                speaker_name: mapping.name.clone(),
                text: segment.text.clone(),
                start_time: segment.start_time,
                end_time: segment.end_time,
                words: segment
                    .words
                    .as_ref()
                    .map(|ws| {
                        ws.iter()
                            .map(|w| WordTimingInput {
                                word: w.word.clone(),
                                start: w.start,
                                end: w.end,
                                score: w.score,
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
            });
        }
    }

    Ok(statements)
}

pub async fn seed_interview(
    ctx: &Context<'_>,
    input: SeedInterviewInput,
) -> async_graphql::Result<SeedInterviewPayload> {
    let db = ctx.data::<Db>()?;

    let interview_uid = nanoid::nanoid!();
    let transcript_uid = nanoid::nanoid!();
    let interviewee_uid = nanoid::nanoid!();

    // ── Phase 1: interview scaffold ──────────────────────────────────────

    // Build interviewer params for UNWIND
    let interviewers: Vec<BoltType> = input
        .speakers
        .iter()
        .filter(|s| s.role == SpeakerRole::Interviewer)
        .map(|s| {
            bolt_map(vec![
                ("name", BoltType::from(s.name.clone())),
                ("uid", BoltType::from(nanoid::nanoid!())),
            ])
        })
        .collect();

    let video_uid = nanoid::nanoid!();
    let video_url = input
        .assets
        .as_ref()
        .and_then(|a| a.video_url.clone())
        .unwrap_or_default();

    db.run(
        query(
            "MERGE (interviewee:Person {name: $intervieweeName})
               ON CREATE SET interviewee.uid = $intervieweeUid

             WITH interviewee
             UNWIND $interviewers AS iv
             MERGE (interviewer:Person {name: iv.name})
               ON CREATE SET interviewer.uid = iv.uid

             WITH interviewee, collect(interviewer) AS interviewers
             CREATE (i:Interview {
               uid: $interviewUid,
               number: $interviewNumber,
               date: date($interviewDate),
               interviewee: $intervieweeName
             })
             CREATE (t:Transcript {uid: $transcriptUid})
             CREATE (i)-[:HAS_TRANSCRIPT]->(t)
             CREATE (i)-[:INTERVIEWS]->(interviewee)

             WITH i, interviewers
             UNWIND interviewers AS interviewer
             CREATE (i)-[:INTERVIEWED_BY]->(interviewer)

             WITH i
             CREATE (v:Video:Asset {uid: $videoUid, url: $videoUrl})
             CREATE (i)-[:HAS_ASSET]->(v)",
        )
        .param("intervieweeName", input.interviewee.clone())
        .param("intervieweeUid", interviewee_uid)
        .param("interviewUid", interview_uid.clone())
        .param("interviewNumber", input.number)
        .param("interviewDate", input.date.clone())
        .param("transcriptUid", transcript_uid.clone())
        .param("interviewers", interviewers)
        .param("videoUid", video_uid.clone())
        .param("videoUrl", video_url),
    )
    .await
    .map_err(gql_err)?;

    // ── Phase 2: seed statements + :NEXT chain ──────────────────────────

    let merged = group_segments(&input)?;
    let statement_count = merged.len() as i64;

    // Batch statements in groups of 100
    const BATCH_SIZE: usize = 100;
    for batch in merged.chunks(BATCH_SIZE) {
        let stmt_params: Vec<BoltType> = batch
            .iter()
            .map(|s| {
                let words_json: BoltType = if s.words.is_empty() {
                    BoltType::Null(neo4rs::BoltNull)
                } else {
                    BoltType::from(
                        serde_json::to_string(&s.words).unwrap_or_default(),
                    )
                };

                bolt_map(vec![
                    ("uid", BoltType::from(nanoid::nanoid!())),
                    ("text", BoltType::from(s.text.clone())),
                    ("startTime", BoltType::from(s.start_time)),
                    ("endTime", BoltType::from(s.end_time)),
                    ("words", words_json),
                    ("speakerName", BoltType::from(s.speaker_name.clone())),
                    ("speakerUid", BoltType::from(nanoid::nanoid!())),
                ])
            })
            .collect();

        db.run(
            query(
                "MATCH (t:Transcript {uid: $transcriptUid})

                 // Find the current tail of the linked list (if any prior batch)
                 OPTIONAL MATCH (t)-[:CONTAINS]->(existing:Statement)
                 WHERE NOT (existing)-[:NEXT]->()
                 WITH t, existing AS prevTail

                 UNWIND range(0, size($statements) - 1) AS idx
                 WITH t, prevTail, idx, $statements[idx] AS s

                 MERGE (person:Person {name: s.speakerName})
                   ON CREATE SET person.uid = s.speakerUid

                 CREATE (stmt:Statement {
                   uid:   s.uid,
                   text:  s.text,
                   words: s.words
                 })
                 CREATE (t)-[:CONTAINS {startTime: s.startTime, endTime: s.endTime}]->(stmt)
                 CREATE (person)-[:SAYS]->(stmt)

                 // Build the :NEXT linked list
                 WITH t, prevTail, collect(stmt) AS stmts
                 FOREACH (i IN range(0, size(stmts) - 2) |
                   FOREACH (a IN [stmts[i]] |
                     FOREACH (b IN [stmts[i + 1]] |
                       CREATE (a)-[:NEXT]->(b)
                     )
                   )
                 )

                 // Link previous batch tail to first new statement
                 WITH prevTail, stmts
                 WHERE prevTail IS NOT NULL AND size(stmts) > 0
                 CREATE (prevTail)-[:NEXT]->(stmts[0])",
            )
            .param("transcriptUid", transcript_uid.clone())
            .param("statements", stmt_params),
        )
        .await
        .map_err(gql_err)?;
    }

    // ── Phase 3: attach caption files (optional) ────────────────────────

    if let Some(assets) = &input.assets {
        let has_vtt = assets.vtt_url.is_some() || assets.vtt_text.is_some();
        let has_json = assets.json_caption_url.is_some() || assets.json_caption_text.is_some();

        if has_vtt {
            db.run(
                query(
                    "MATCH (v:Video:Asset {uid: $videoUid})
                     CREATE (vtt:VTT:Caption {uid: $vttUid, url: $vttUrl, text: $vttText})
                     CREATE (v)-[:HAS_CAPTIONS]->(vtt)",
                )
                .param("videoUid", video_uid.clone())
                .param("vttUid", nanoid::nanoid!())
                .param("vttUrl", assets.vtt_url.clone().unwrap_or_default())
                .param("vttText", assets.vtt_text.clone().unwrap_or_default()),
            )
            .await
            .map_err(gql_err)?;
        }

        if has_json {
            db.run(
                query(
                    "MATCH (v:Video:Asset {uid: $videoUid})
                     CREATE (json:JSON:Caption {uid: $jsonUid, url: $jsonUrl, text: $jsonText})
                     CREATE (v)-[:HAS_CAPTIONS]->(json)",
                )
                .param("videoUid", video_uid)
                .param("jsonUid", nanoid::nanoid!())
                .param("jsonUrl", assets.json_caption_url.clone().unwrap_or_default())
                .param(
                    "jsonText",
                    assets.json_caption_text.clone().unwrap_or_default(),
                ),
            )
            .await
            .map_err(gql_err)?;
        }
    }

    // ── Build response ──────────────────────────────────────────────────

    let speaker_count = input.speakers.len() as i64;

    Ok(SeedInterviewPayload {
        interview: Interview {
            uid: interview_uid,
            number: input.number,
            interviewee: input.interviewee,
            date: input.date,
        },
        statement_count,
        speaker_count,
        transcript_uid,
    })
}
