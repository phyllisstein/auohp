use async_graphql::{Context, SimpleObject};

// Each struct below mirrors one node label in the Neo4j graph.
// The mapping is deliberately one-to-one so that changes to the graph model
// surface immediately as compilation errors here, rather than silently
// surviving as stale projection queries.
//
// Relationships with properties (TRANSCRIBES) become intermediate GraphQL
// types (TranscriptEntry), since GraphQL has no native concept of a
// property-bearing edge. Relationships without properties (HAS_TRANSCRIPT,
// SAYS, INTERVIEWED_AS) become nested fields on the source type.

/// Mirrors the (:Person) node.
#[derive(SimpleObject)]
pub struct Person {
    pub uid: String,
    pub name: String,
}

/// Mirrors the (:Speaker) node — the role a Person plays in a given interview.
///
/// Note: the Speaker indirection is acknowledged as clumsy in CLAUDE.md.
/// The planned future model is (:Statement)<-[:SAYS]-(:Person) directly.
/// This type will be removed when that migration happens.
#[derive(SimpleObject)]
pub struct Speaker {
    /// "Interviewer" or "Interviewee".
    pub label: String,
    /// The Person who occupies this speaker role (via INTERVIEWED_AS).
    pub person: Person,
}

/// Mirrors the (:Statement) node.
#[derive(SimpleObject)]
pub struct Statement {
    pub uid: String,
    pub text: String,
    /// The speaker who uttered this statement (via SAYS → INTERVIEWED_AS).
    pub speaker: Speaker,
}

/// Represents the [TRANSCRIBES] relationship and its target Statement.
///
/// In GraphQL, a relationship with properties has no direct equivalent, so
/// we promote it to a first-class type. `TranscriptEntry` carries the timing
/// data that lives on the TRANSCRIBES edge plus a reference to the Statement
/// node that edge points at.
#[derive(SimpleObject)]
pub struct TranscriptEntry {
    /// Seconds from the start of the recording.
    pub start_time: f64,
    pub end_time: f64,
    /// Human-readable timestamp string, e.g. "00:04:32.500".
    pub start_timestamp: String,
    pub end_timestamp: String,
    pub statement: Statement,
}

/// Mirrors the (:Transcript) node, with its ordered entries and the
/// Interview it belongs to.
#[derive(SimpleObject)]
pub struct Transcript {
    pub uid: String,
    /// The interview this transcript belongs to (via HAS_TRANSCRIPT).
    pub interview: Interview,
    /// Statements in chronological order (sorted by TRANSCRIBES.startTime).
    pub entries: Vec<TranscriptEntry>,
}

/// Mirrors the (:Interview) node.
#[derive(SimpleObject)]
pub struct Interview {
    pub uid: String,
    pub number: i64,
    pub interviewee: String,
    /// ISO 8601 date string, e.g. "1995-04-23".
    pub date: String,
}

// ---------------------------------------------------------------------------
// Resolver functions — called from QueryRoot in mod.rs.
// ---------------------------------------------------------------------------

pub async fn list_interviews(_ctx: &Context<'_>) -> async_graphql::Result<Vec<Interview>> {
    todo!("Neo4j queries wired up in the next commit")
}

pub async fn get_transcript(
    _ctx: &Context<'_>,
    _number: i64,
) -> async_graphql::Result<Transcript> {
    todo!("Neo4j queries wired up in the next commit")
}
