# Neo4j Schema Design for AUOHP Seed Layer

> **Superseded 2026-08-21** --- implemented in
> `packages/api/src/graphql/mutations/seed_interview.rs`. The headline decision
> (§2.1, dropping the `:Speaker` indirection) shipped: the seed mutation now
> writes `(person)-[:SAYS]->(statement)` directly. Retained for the decision
> record. Note the doc refers to `packages/auohp-api`, since renamed to
> `packages/api`. The pre-existing `:Speaker` model survives only in
> `packages/scripts/*.ts`, which this schema was written to replace.

## 1. Overview

This document proposes a clean Neo4j graph schema for the ACT UP Oral History Project, resolving the FIXMEs in the existing `whisper-to-neo4j.ts` seed script. The new schema will be consumed by a Rust seed endpoint in `packages/auohp-api`.

---

## 2. Design Decisions

### 2.1 Remove the :Speaker Indirection Layer

**Decision: Remove `:Speaker` nodes entirely.**

Current model:
```
(:Person)-[:INTERVIEWED_AS]->(:Speaker)-[:SAYS]->(:Statement)
```

Proposed model:
```
(:Person)-[:SAYS]->(:Statement)
```

**Rationale:**

- The `:Speaker` node exists solely to carry a `label` field (e.g. `"SPEAKER_01"`) -- a diarization artifact that has no meaning outside the transcription pipeline. Once the speaker is identified as a `Person`, the label is no longer needed.
- The role a person plays in an interview (interviewer vs. interviewee) is better expressed as a relationship property or a separate relationship type on the `Interview` node -- not as a node in its own right.
- The `:INCLUDES_SPEAKER` edge on `Transcript` was only needed to look up speakers by label during seeding. In the new model, speakers are resolved to `Person` nodes before seeding.

**What changes in existing queries:**

The `get_transcript` query in `interviews.rs` currently traverses:
```cypher
(transcript)-[transcribes:TRANSCRIBES]->(statement)<-[:SAYS]-(speaker)<-[:INTERVIEWED_AS]-(person:Person)
```

This simplifies to:
```cypher
(transcript)-[c:CONTAINS]->(statement)<-[:SAYS]-(person:Person)
```

The `speaker.label` field in the GraphQL `Speaker` type disappears. Instead, the `Person` is returned directly, and the role (interviewer/interviewee) is available from the `Interview` node's relationships.

**Modeling interviewer/interviewee roles:**

Replace `:INTERVIEWED_AS` with direct relationships on the `Interview` node:
```
(:Interview)-[:INTERVIEWED_BY]->(:Person)   // interviewer(s)
(:Interview)-[:INTERVIEWS]->(:Person)       // interviewee
```

This generalizes the hardcoded Jim Hubbard / Sarah Schulman problem. Any number of interviewers can be linked to any interview. The `Interview.interviewee` string property is kept for convenience but the canonical source is the `:INTERVIEWS` edge.

### 2.2 Replace :TRANSCRIBES with :CONTAINS + :NEXT Linked List

**Decision: Rename `:TRANSCRIBES` to `:CONTAINS` and use `:NEXT` edges between Statements for ordering.**

Current model has *two* edges to each Statement:
```
(transcript)-[:TRANSCRIBES {timing...}]->(statement)
(speaker)-[:SAYS {timing...}]->(statement)
```

Both carry identical timing properties. This is the duplication flagged by the FIXME.

Proposed model:
```
(transcript)-[:CONTAINS {startTime?, endTime?}]->(statement)
(statement)-[:NEXT]->(nextStatement)
(person)-[:SAYS]->(statement)
```

- Timing stays on the `:CONTAINS` edge as optional properties (see section 2.3). Not all Statements originate from a timed transcription -- broadsheet text, for example, has no media timing. Keeping timing on the edge rather than the node means untimed Statements are not forced to carry null timing fields.
- `:CONTAINS` is a flat fan-out from Transcript to all its Statements (no ordering property).
- `:NEXT` edges form a singly-linked list between consecutive Statements within a Transcript, providing explicit ordering without an integer index. This is idiomatic for Neo4j ordered sequences and makes insertion/reordering a local operation (repoint two edges) rather than a renumbering of all subsequent indices.
- The first Statement in a Transcript is the one with no inbound `:NEXT` edge from another Statement in the same Transcript.
- `:SAYS` becomes a simple edge with no properties -- it just links a person to what they said.

### 2.3 Timing: Float Seconds on :CONTAINS Edge, Not Duration

**Decision: Keep timing as `f64` seconds, stored as optional properties on the `:CONTAINS` edge.**

The existing FIXME suggests using Neo4j's `Duration` type. However:

- `Duration` is designed for calendar-aware intervals (years, months, days, hours). Media timestamps are simple floating-point offsets from zero.
- `Duration` has no sub-second precision beyond milliseconds in its ISO 8601 representation. WhisperX provides timings to ~10ms, but future pipelines may provide finer granularity.
- Arithmetic on floats (`rel.startTime + 5.0`) is simpler and more portable than duration arithmetic.
- The Rust `neo4rs` crate has better support for `f64` than for Neo4j temporal types.

Timing lives on the `:CONTAINS` edge, not the Statement node, because not all Statements have a media timestamp. Broadsheet transcriptions, documentary narration text, and other non-timed sources produce Statements with no timing. Putting timing on the edge makes it an attribute of the relationship between a specific Transcript and a Statement, which is semantically correct: the same text could theoretically appear in different contexts with different timings.

The `:CONTAINS` edge optionally carries:
- `startTime: Float` -- seconds from start of recording
- `endTime: Float` -- seconds from start of recording

Human-readable timestamps (e.g. `"00:04:32.500"`) are **not stored** -- they are trivially derived from the float and can be formatted in the API layer. This eliminates a class of consistency bugs where the float and the string disagree.

### 2.4 Generalizing Interviewers

**Decision: Remove hardcoded interviewer names. Interviewers are input data, not schema constants.**

Currently `bootstrap()` creates Jim Hubbard and Sarah Schulman as `Person` nodes before any interviews are seeded. The `seedInterview` function then `MATCH`es them by name.

In the new model:
- The seed endpoint receives a list of participants (name + role) as input.
- `Person` nodes are `MERGE`d by name (or a stable external ID if available).
- `:INTERVIEWED_BY` and `:INTERVIEWS` edges are created per-interview.

This means the same person appearing as interviewer in one interview and interviewee in another (unlikely but possible) is handled correctly -- the role is on the relationship, not the node.

### 2.5 Per-Word Timing Data

**Decision: Store words as a JSON string property on Statement, not as child nodes.**

Options considered:
1. **Child `:Word` nodes** -- One node per word with `(statement)-[:HAS_WORD {order}]->(word:Word {text, start, end})`. This would create ~100-500 nodes per statement and ~50,000+ nodes per interview. The graph overhead is extreme for data that is only ever consumed as a flat list.
2. **JSON string property** -- `statement.words = '[{"word":"the","start":1.23,"end":1.45}, ...]'`. Compact, easy to deserialize in Rust/JS. Not individually queryable in Cypher, but per-word search is not a requirement.
3. **Neo4j list-of-maps** -- Similar to JSON but uses Neo4j's native list type. Slightly better for Cypher access but harder to work with in `neo4rs`.

**Chosen: Option 2 (JSON string).** The words array is an opaque payload used by the caption editor for word-level highlighting and timing. It does not need to be traversed or filtered in Cypher. Storing it as a JSON string keeps the node count manageable and avoids schema complexity.

If per-word search becomes a requirement in the future, a full-text index on `Statement.text` (which already exists) covers the word-search use case, and the word timings can be used client-side to highlight matches.

---

## 3. Proposed Schema

### 3.1 Node Labels and Properties

```
(:Interview {
    uid:          String    // nanoid
    number:       Integer   // interview number in the archive
    date:         Date      // Neo4j Date type
    interviewee:  String    // convenience field, canonical source is :INTERVIEWS edge
})

(:Person {
    uid:   String   // nanoid
    name:  String   // full name
})

(:Transcript {
    uid: String     // nanoid
})

(:Statement {
    uid:            String   // nanoid
    text:           String   // full text of the statement
    words:          String   // JSON array of {word, start, end} objects (optional)
})

(:Video:Asset {
    uid: String
    url: String
})

(:VTT:Caption {
    uid:  String
    url:  String
    text: String   // full VTT content
})

(:JSON:Caption {
    uid:  String
    url:  String
    text: String   // full JSON content
})

(:Action {
    uid:  String
    name: String
    date: Date
})

(:Broadsheet {
    uid:   String
    title: String
})

(:Documentary:Film {
    uid:   String
    title: String
    date:  Date
    slug:  String
})
```

### 3.2 Relationships

```
// Interview structure
(:Interview)-[:HAS_TRANSCRIPT]->(:Transcript)
(:Interview)-[:INTERVIEWS]->(:Person)        // the interviewee
(:Interview)-[:INTERVIEWED_BY]->(:Person)    // interviewer(s)
(:Interview)-[:HAS_ASSET]->(:Video)

// Transcript membership (timing is optional -- only present for media-backed statements)
(:Transcript)-[:CONTAINS {startTime?: Float, endTime?: Float}]->(:Statement)

// Statement ordering (singly-linked list within a transcript)
(:Statement)-[:NEXT]->(:Statement)

// Attribution
(:Person)-[:SAYS]->(:Statement)

// Media assets
(:Video)-[:HAS_CAPTIONS]->(:VTT|:JSON:Caption)

// Actions and broadsheets (existing, unchanged)
(:Action)-[:HAS_DOCUMENTARY]->(:Documentary)
(:Action)-[:HAS_BROADSHEET]->(:Broadsheet)
(:Broadsheet)-[:MENTIONS]->(:Action)
(:Documentary)-[:MENTIONS]->(:Action)
(:Statement)-[:MENTIONS]->(:Action)
(:Broadsheet)-[:HAS_ASSET]->(:Image:Asset)
(:Broadsheet)-[:HAS_TRANSCRIPT]->(:Transcript)
(:Collective)-[:DESIGNS]->(:Broadsheet)
```

### 3.3 Visual Diagram

```
                    ┌──────────┐
         ┌─INTERVIEWS──▶│  Person  │◀─INTERVIEWED_BY─┐
         │               └──────────┘                  │
         │                    │                        │
         │                   SAYS                      │
         │                    ▼                        │
  ┌──────────┐    ┌──────────────┐    ┌───────────┐       ┌───────────┐
  │Interview │─HAS_TRANSCRIPT──▶│  Transcript  │─CONTAINS──▶│Statement 1│──NEXT──▶│Statement 2│──NEXT──▶ ...
  └──────────┘                  └──────────────┘           └───────────┘       └───────────┘
       │
    HAS_ASSET
       ▼
  ┌──────────┐
  │  Video   │─HAS_CAPTIONS──▶ VTT / JSON Caption
  └──────────┘
```

---

## 4. Seed Cypher

### 4.1 Bootstrap: Constraints and Indexes

```cypher
-- Uniqueness constraints (one per label that has a uid)
CREATE CONSTRAINT InterviewUID IF NOT EXISTS
  FOR (n:Interview) REQUIRE n.uid IS UNIQUE;

CREATE CONSTRAINT PersonUID IF NOT EXISTS
  FOR (n:Person) REQUIRE n.uid IS UNIQUE;

CREATE CONSTRAINT TranscriptUID IF NOT EXISTS
  FOR (n:Transcript) REQUIRE n.uid IS UNIQUE;

CREATE CONSTRAINT StatementUID IF NOT EXISTS
  FOR (n:Statement) REQUIRE n.uid IS UNIQUE;

CREATE CONSTRAINT VideoUID IF NOT EXISTS
  FOR (n:Video) REQUIRE n.uid IS UNIQUE;

CREATE CONSTRAINT VTTUID IF NOT EXISTS
  FOR (n:VTT) REQUIRE n.uid IS UNIQUE;

-- Full-text search indexes
CREATE FULLTEXT INDEX transcript_search IF NOT EXISTS
  FOR (s:Statement) ON EACH [s.text];

CREATE FULLTEXT INDEX name_search IF NOT EXISTS
  FOR (p:Person) ON EACH [p.name];

-- Vector index (for embedding-based search)
CREATE VECTOR INDEX statement_embedding IF NOT EXISTS
  FOR (s:Statement) ON s.embedding
  OPTIONS {indexConfig: {
    `vector.dimensions`: 1536,
    `vector.similarity_function`: 'cosine'
  }};
```

### 4.2 Seed an Interview

The Rust seed endpoint will receive a payload containing interview metadata, participant info, and the transcript segments. The seeding happens in three phases.

**Phase 1: Create interview scaffold**

```cypher
// MERGE persons by name so re-seeding is idempotent
MERGE (interviewee:Person {name: $intervieweeName})
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
CREATE (v:Video:Asset {uid: $videoUid, url: $videoUrl})
CREATE (i)-[:HAS_TRANSCRIPT]->(t)
CREATE (i)-[:HAS_ASSET]->(v)
CREATE (i)-[:INTERVIEWS]->(interviewee)

WITH i, interviewers
UNWIND interviewers AS interviewer
CREATE (i)-[:INTERVIEWED_BY]->(interviewer)
```

**Phase 2: Seed statements and build the :NEXT chain**

Statements are seeded in order. Each batch creates Statement nodes, links them to the Transcript via `:CONTAINS` (with optional timing), links them to their speaker via `:SAYS`, and wires up `:NEXT` edges. The `$statements` list must be in transcript order.

```cypher
MATCH (t:Transcript {uid: $transcriptUid})

// Find the current tail of the linked list (if any prior batch was seeded)
OPTIONAL MATCH (t)-[:CONTAINS]->(existing:Statement)
WHERE NOT (existing)-[:NEXT]->()
WITH t, existing AS prevTail

// Create each statement and chain them
WITH t, prevTail
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

// Build the linked list: first statement links from prevTail (if exists),
// subsequent statements link from the previous one in this batch
WITH t, prevTail, collect(stmt) AS stmts
FOREACH (i IN range(0, size(stmts) - 2) |
  FOREACH (a IN [stmts[i]] |
    FOREACH (b IN [stmts[i + 1]] |
      CREATE (a)-[:NEXT]->(b)
    )
  )
)

// Link prevTail to first new statement if this is a continuation batch
WITH prevTail, stmts
WHERE prevTail IS NOT NULL AND size(stmts) > 0
CREATE (prevTail)-[:NEXT]->(stmts[0])
```

For untimed statements (e.g. broadsheet text), omit `startTime` and `endTime` from the `$statements` entries -- the `:CONTAINS` edge will simply have no timing properties.

**Phase 3: Attach caption files (optional)**

```cypher
MATCH (v:Video:Asset {uid: $videoUid})
CREATE (vtt:VTT:Caption {uid: $vttUid, url: $vttUrl, text: $vttText})
CREATE (json:JSON:Caption {uid: $jsonUid, url: $jsonUrl, text: $jsonText})
CREATE (v)-[:HAS_CAPTIONS]->(vtt)
CREATE (v)-[:HAS_CAPTIONS]->(json)
```

### 4.3 Updated Read Queries

**List interviews:**

```cypher
MATCH (i:Interview)
RETURN i.uid AS uid,
       i.number AS number,
       i.interviewee AS interviewee,
       toString(i.date) AS date
ORDER BY i.date ASC
```

(Unchanged from current.)

**Get transcript (ordered via :NEXT chain):**

```cypher
MATCH (i:Interview {number: $number})-[:HAS_TRANSCRIPT]->(t:Transcript)

// Find the head of the linked list (a Statement with no inbound :NEXT from this transcript)
MATCH (t)-[:CONTAINS]->(head:Statement)
WHERE NOT EXISTS {
  MATCH (t)-[:CONTAINS]->(prev:Statement)-[:NEXT]->(head)
}

// Walk the :NEXT chain to collect statements in order
MATCH path = (head)-[:NEXT*0..]->(s:Statement)
WHERE (t)-[:CONTAINS]->(s)
WITH i, t, s, length(path) AS pos

MATCH (t)-[c:CONTAINS]->(s)<-[:SAYS]-(p:Person)
OPTIONAL MATCH (i)-[:INTERVIEWED_BY]->(interviewer:Person)
  WHERE interviewer = p

RETURN i.uid           AS interview_uid,
       i.number        AS interview_number,
       i.interviewee   AS interviewee,
       toString(i.date) AS interview_date,
       t.uid           AS transcript_uid,
       s.uid           AS statement_uid,
       s.text          AS statement_text,
       c.startTime     AS start_time,
       c.endTime       AS end_time,
       s.words         AS words,
       p.uid           AS person_uid,
       p.name          AS person_name,
       interviewer IS NOT NULL AS is_interviewer
ORDER BY pos
```

Key changes from the current query:
- Timing comes from `:CONTAINS` edge properties (optional -- null for non-media statements), not duplicated `:TRANSCRIBES` edge properties.
- `speaker.label` is gone; replaced by `person.name` and a boolean `is_interviewer`.
- Ordering uses the `:NEXT` linked list traversal instead of `ORDER BY startTime` (which can have ties) or an integer index.
- No `startTimestamp`/`endTimestamp` strings -- these are derived in the API layer.

---

## 5. Index and Constraint Recommendations

| Type | Name | Target | Purpose |
|------|------|--------|---------|
| Uniqueness | `InterviewUID` | `Interview.uid` | Prevent duplicate interviews |
| Uniqueness | `PersonUID` | `Person.uid` | Prevent duplicate persons |
| Uniqueness | `StatementUID` | `Statement.uid` | Prevent duplicate statements |
| Uniqueness | `TranscriptUID` | `Transcript.uid` | Prevent duplicate transcripts |
| Uniqueness | `VideoUID` | `Video.uid` | Prevent duplicate videos |
| Full-text | `transcript_search` | `Statement.text` | Search across all statements |
| Full-text | `name_search` | `Person.name` | Search for people by name |
| Vector | `statement_embedding` | `Statement.embedding` | Semantic similarity search |
| Composite index | -- | `Interview.number` | Fast lookup by interview number (implicit from uniqueness if we add a constraint) |

**Recommendation:** Add a uniqueness constraint on `Interview.number` since it is the primary lookup key in the GraphQL API:

```cypher
CREATE CONSTRAINT InterviewNumber IF NOT EXISTS
  FOR (n:Interview) REQUIRE n.number IS UNIQUE;
```

---

## 6. Migration Path

The existing TypeScript seed script (`whisper-to-neo4j.ts`) does a full `MATCH (n) DETACH DELETE n` before re-seeding. The Rust seed endpoint should follow the same pattern initially: clear and re-seed. This avoids complex migration logic.

For production, the seed endpoint should support incremental seeding:
- `MERGE` on `Interview.number` instead of `CREATE`
- `MERGE` on `Person.name`
- Delete existing statements for a transcript before re-seeding (to handle re-transcription)

---

## 7. Transcript JSON Format Dependency

This schema assumes the transcription pipeline (Task #1) produces segments in a format like:

```json
{
  "segments": [
    {
      "speaker": "Sarah Schulman",
      "text": "Tell me about your involvement with ACT UP.",
      "start": 12.34,
      "end": 15.67,
      "words": [
        {"word": "Tell", "start": 12.34, "end": 12.56},
        {"word": "me", "start": 12.57, "end": 12.68}
      ]
    }
  ]
}
```

Key assumptions:
- **Speaker labels are resolved to names before seeding.** The diarization pipeline outputs opaque labels like `SPEAKER_01`; the user maps these to real names (or the pipeline does so via a lookup table). The Neo4j schema does not store diarization labels.
- **Segments are already merged by speaker.** Adjacent segments from the same speaker are concatenated into a single statement by the pipeline (or by the seed layer). Each `Statement` node represents a contiguous block of speech by one person.
- **Words array is optional.** If the pipeline does not produce per-word timing, the `words` property is omitted (or set to `null`).
- **Timing is optional.** Non-media sources (broadsheets, flyers) produce Statements with text but no `start`/`end` fields. The `:CONTAINS` edge for these statements simply lacks timing properties.

**Open question for Task #1:** Should the transcript JSON include the speaker-to-name mapping, or should that be provided separately as interview metadata? The seed endpoint currently expects interview metadata (participants, date, number) to be provided separately from the transcript segments. The transcript JSON should include speaker labels, and the mapping from labels to names is part of the interview metadata.

---

## 8. Summary of Changes from Current Schema

| Aspect | Current | Proposed |
|--------|---------|----------|
| Speaker model | `(Person)-[:INTERVIEWED_AS]->(Speaker)-[:SAYS]->(Statement)` | `(Person)-[:SAYS]->(Statement)` |
| Interviewer role | Hardcoded Jim/Sarah as `Person` nodes + `Speaker:Interviewer` label | `(Interview)-[:INTERVIEWED_BY]->(Person)` |
| Statement timing | On `:TRANSCRIBES` and `:SAYS` edges (duplicated) | Optional properties on `:CONTAINS` edge |
| Timing format | Float + human-readable string | Float only (on edge), absent for non-media statements |
| Transcript-to-Statement edge | `:TRANSCRIBES` with timing properties | `:CONTAINS` with optional timing |
| Statement ordering | `ORDER BY startTime` | `:NEXT` linked list between Statements |
| Per-word data | Not persisted | `Statement.words` JSON string property |
| Node count per interview | ~N statements + ~3 speakers + edges | ~N statements (speakers are shared `Person` nodes) |
