# GraphQL Mutation Design: Interview Seeding

## Status

Design document -- no Rust code yet. Depends on:

- **Task #1** (transcription pipeline design) for the exact shape of the WhisperX JSON that the Rust API will ingest.
- **Task #2** (Neo4j seed schema / Cypher) for the Cypher statements the resolvers will execute.

---

## 1. Overview

The current `AppSchema` is `Schema<QueryRoot, EmptyMutation, EmptySubscription>`. This document proposes the `MutationRoot` that replaces `EmptyMutation`, along with the `InputObject` types it accepts and the `SimpleObject` result types it returns.

### Goals

1. Accept a complete interview's worth of data in a single GraphQL call (metadata + speakers + transcript segments).
2. Generalize speaker resolution -- no hardcoded names.
3. Group raw word-timed segments into statements server-side (in Rust), so the client sends the WhisperX output as-is.
4. Return enough data for the caller to verify what was created.

---

## 2. Mutation Root

```rust
use async_graphql::{Context, Object};

pub struct MutationRoot;

#[Object]
impl MutationRoot {
    /// Seeds a complete interview: creates the Interview, Person,
    /// Speaker, Transcript, and Statement nodes plus all relationships.
    async fn seed_interview(
        &self,
        ctx: &Context<'_>,
        input: SeedInterviewInput,
    ) -> async_graphql::Result<SeedInterviewPayload> {
        mutations::seed_interview(ctx, input).await
    }
}
```

### Why one big mutation instead of composable pieces?

The existing TypeScript seeder (`whisper-to-neo4j.ts`) performs the entire seed in a single logical operation: create the interview node, create speakers, then batch-create statements. Splitting this into `seedPerson` / `seedInterview` / `addTranscript` would force the client to orchestrate ordering and error recovery across multiple round-trips. Since the primary caller is an automated pipeline (not an interactive UI), a single `seedInterview` mutation that creates everything atomically is the right fit.

If finer-grained mutations are needed later (e.g., appending a transcript to an existing interview, or correcting a person's name), they can be added alongside `seedInterview` without breaking it.

### Wiring into AppSchema

In `schema.rs`:

```rust
use super::mutations::MutationRoot;

pub type AppSchema = Schema<QueryRoot, MutationRoot, EmptySubscription>;

pub fn build_schema(db: Db) -> AppSchema {
    Schema::build(QueryRoot, MutationRoot, EmptySubscription)
        .data(db)
        .finish()
}
```

---

## 3. Input Types

All input types derive `async_graphql::InputObject`.

### 3.1 Top-level: `SeedInterviewInput`

```rust
#[derive(InputObject)]
pub struct SeedInterviewInput {
    /// Interview number in the AUOHP archive (e.g. 25, 64, 82).
    pub number: i64,
    /// ISO 8601 date string, e.g. "2003-05-05".
    pub date: String,
    /// Display name for the interviewee (e.g. "Lei Chou").
    pub interviewee: String,
    /// Participants in the interview. Each entry maps a diarization
    /// speaker label (e.g. "SPEAKER_01") to a person name and role.
    pub speakers: Vec<SpeakerMappingInput>,
    /// The raw transcript segments from WhisperX output.
    /// The server groups consecutive same-speaker segments into
    /// statements and creates the graph nodes accordingly.
    pub segments: Vec<TranscriptSegmentInput>,
    /// Optional asset URLs associated with the interview.
    pub assets: Option<InterviewAssetsInput>,
}
```

### 3.2 Speaker mapping: `SpeakerMappingInput`

This replaces the hardcoded `{ interviewee: "SPEAKER_01", jim: "SPEAKER_03", sarah: "SPEAKER_02" }` pattern from the TypeScript seeder with a general model.

```rust
#[derive(InputObject)]
pub struct SpeakerMappingInput {
    /// The diarization label from WhisperX (e.g. "SPEAKER_00").
    pub label: String,
    /// The person's display name (e.g. "Jim Hubbard", "Sarah Schulman").
    pub name: String,
    /// The role this person plays: "Interviewer" or "Interviewee".
    pub role: SpeakerRole,
}

#[derive(async_graphql::Enum, Copy, Clone, Eq, PartialEq)]
pub enum SpeakerRole {
    Interviewer,
    Interviewee,
}
```

**Design rationale**: The current TypeScript code hardcodes Jim Hubbard and Sarah Schulman as interviewers by matching them in the bootstrap phase and then referencing them via `MATCH (jim:Person {name: 'Jim Hubbard'})`. This is fragile -- it breaks if a different interviewer conducts a session, or if Person nodes do not already exist.

The proposed `SpeakerMappingInput` is fully general:

- Any number of speakers with any role.
- The mutation resolver does a `MERGE` on `(:Person {name: $name})` so that recurring persons (like Jim and Sarah) are found-or-created automatically.
- The `SpeakerRole` enum replaces the Neo4j `:Interviewer`/`:Interviewee` secondary labels on Speaker nodes.

### 3.3 Transcript segment: `TranscriptSegmentInput`

This matches the WhisperX JSON output format -- a flat list of timed, speaker-attributed text chunks.

```rust
#[derive(InputObject)]
pub struct TranscriptSegmentInput {
    /// The transcribed text for this segment.
    pub text: String,
    /// Start time in seconds from the beginning of the recording.
    pub start_time: f64,
    /// End time in seconds.
    pub end_time: f64,
    /// Human-readable start timestamp, e.g. "00:04:32.500".
    pub start_timestamp: String,
    /// Human-readable end timestamp.
    pub end_timestamp: String,
    /// Diarization speaker label (e.g. "SPEAKER_01").
    /// Must match one of the labels in SeedInterviewInput.speakers.
    pub speaker: String,
    /// Per-word timing data from WhisperX. Optional because some
    /// segments may lack word-level alignment.
    pub words: Option<Vec<WordTimingInput>>,
}
```

### 3.4 Word-level timing: `WordTimingInput`

```rust
#[derive(InputObject)]
pub struct WordTimingInput {
    pub word: String,
    pub start: f64,
    pub end: f64,
    /// WhisperX confidence score (0.0 -- 1.0).
    pub score: Option<f64>,
}
```

### 3.5 Optional assets: `InterviewAssetsInput`

```rust
#[derive(InputObject)]
pub struct InterviewAssetsInput {
    /// URL to the interview video file.
    pub video_url: Option<String>,
    /// URL to the WebVTT caption file.
    pub vtt_url: Option<String>,
    /// Raw WebVTT text content, if available at seed time.
    pub vtt_text: Option<String>,
    /// URL to the JSON captions file.
    pub json_caption_url: Option<String>,
    /// Raw JSON captions text, if available at seed time.
    pub json_caption_text: Option<String>,
}
```

---

## 4. Result / Payload Type

The mutation returns enough data to confirm what was created:

```rust
#[derive(SimpleObject)]
pub struct SeedInterviewPayload {
    /// The created Interview node.
    pub interview: Interview,
    /// Number of Statement nodes created.
    pub statement_count: i64,
    /// Number of Speaker nodes created or matched.
    pub speaker_count: i64,
    /// The uid of the created Transcript node.
    pub transcript_uid: String,
}
```

This reuses the existing `Interview` output type from `interviews.rs`. The payload intentionally does not return the full transcript (which could be thousands of statements) -- the caller can follow up with the existing `interviewTranscript` query if needed.

---

## 5. Server-Side Statement Grouping

**Decision: the mutation accepts raw segments and groups them in Rust.**

The TypeScript seeder already performs this grouping (lines 160-187 of `whisper-to-neo4j.ts`): consecutive segments with the same `speaker` label are merged into a single Statement node. The Rust resolver will replicate this logic:

```
for each segment in input.segments (ordered by start_time):
    if segment.speaker != current_speaker:
        flush current statement to Neo4j
        start new statement
    append segment text to current statement
    extend current statement's time range
    collect word timings
flush final statement
```

This keeps the client simple (just send WhisperX output) and ensures that statement boundaries are consistent regardless of which client calls the mutation.

---

## 6. Speaker Resolution Strategy

The resolver performs the following for each entry in `input.speakers`:

1. `MERGE (person:Person {name: $name})` -- find or create the Person node.
2. `CREATE (speaker:Speaker {label: $label})` -- always create a new Speaker node per interview (a Speaker is the role a Person plays *in a specific interview*).
3. Apply the `:Interviewer` or `:Interviewee` secondary label based on `role`.
4. `MERGE (person)-[:INTERVIEWED_AS]->(speaker)`.
5. `CREATE (transcript)-[:INCLUDES_SPEAKER]->(speaker)`.

This means:
- Jim Hubbard and Sarah Schulman are found by `MERGE` on subsequent seeds (no pre-seeding required).
- A new interviewer can participate without code changes.
- The interviewee Person node is created fresh each time (since they are unique per interview).

---

## 7. Relationship to Existing Output Types

| Output type (interviews.rs) | Created by mutation | Notes |
|---|---|---|
| `Interview` | Yes | Returned in payload |
| `Transcript` | Yes | uid returned in payload |
| `TranscriptEntry` | Yes (one per grouped Statement) | Queryable via `interviewTranscript` |
| `Statement` | Yes (one per grouped Statement) | |
| `Speaker` | Yes (one per speaker mapping) | |
| `Person` | Merged (find-or-create) | |

The mutation creates exactly the graph structure that the existing `get_transcript` query expects: `(Interview)-[:HAS_TRANSCRIPT]->(Transcript)-[:TRANSCRIBES]->(Statement)<-[:SAYS]-(Speaker)<-[:INTERVIEWED_AS]-(Person)`.

---

## 8. Error Handling

- If any `segment.speaker` label does not appear in `input.speakers`, the mutation returns a validation error before touching Neo4j.
- The entire seed operation should run inside a Neo4j transaction. If any Cypher statement fails, the transaction rolls back and the mutation returns an error.
- Duplicate interview numbers are caught by the uniqueness constraint on `Interview.uid` (or a pre-check on `Interview.number`).

---

## 9. Future Extensions

These are not part of the initial implementation but noted as natural extension points:

- **`updateStatement` mutation**: For the caption editor's save operation (currently `PUT /api/transcript`). Would accept a statement uid and updated text/timing.
- **`deleteInterview` mutation**: For re-seeding. Would detach-delete the entire subgraph.
- **`seedVectorIndex` mutation**: To trigger embedding generation for statements after seeding.

---

## 10. File Organization

New files under `src/graphql/`:

```
src/graphql/
  mod.rs              -- add `mod mutations;`
  mutations/
    mod.rs            -- pub use, MutationRoot
    seed_interview.rs -- SeedInterviewInput, resolver logic
    types.rs          -- SpeakerMappingInput, TranscriptSegmentInput, etc.
  interviews.rs       -- (existing, unchanged)
  schema.rs           -- update AppSchema type alias
```

---

## 11. Dependencies on Other Tasks

| Dependency | What we need | Impact if different |
|---|---|---|
| Task #1 (transcription pipeline) | Exact field names in WhisperX JSON output | `TranscriptSegmentInput` and `WordTimingInput` field names may need adjustment |
| Task #2 (Neo4j Cypher design) | The Cypher statements for creating nodes/relationships | The resolver implementation will call whatever Cypher Task #2 produces; input types should be stable regardless |
