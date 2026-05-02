//! Domain types shared across all REST handlers.
//!
//! These structs mirror the Neo4j graph model closely --- one struct per node
//! label, with relationship properties lifted onto the target type as optional
//! fields where needed (e.g. timing from `:CONTAINS` lives on `Statement`).
//!
//! Previously these lived under `graphql/interviews.rs` and carried
//! `async_graphql::SimpleObject` derives. The GraphQL derive macros are gone;
//! in their place each type gets `serde::Serialize` (for JSON HTTP responses)
//! and `serde::Deserialize` (for neo4rs node deserialization via
//! `node.to::<T>()`). The two derive paths share the same field names, so no
//! `#[serde(rename)]` is needed as long as Cypher property names match Rust
//! field names.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Node types
// ---------------------------------------------------------------------------

/// Mirrors the (:Person) node.
///
/// `Deserialize` lets neo4rs turn a Bolt `Node` directly into this struct via
/// `node.to::<Person>()`. Field names (`uid`, `name`) match Neo4j property
/// names exactly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Person {
    pub uid: String,
    pub name: String,
}

/// The node-native properties of a (:Statement) node, separated from the
/// full `Statement` type because `Statement` also carries data from
/// relationships (`:CONTAINS` timing, `:SAYS` speaker), which prevents a
/// single `Deserialize` derive over the whole thing.
///
/// `#[serde(default)]` on `words` means serde substitutes `None` for absent
/// keys rather than erroring --- older transcripts were seeded without
/// word-level timing, so the property may not exist on the node at all.
#[derive(Deserialize)]
pub struct StatementNode {
    pub uid: String,
    pub text: String,
    #[serde(default)]
    pub words: Option<String>,
}

/// Mirrors the (:Statement) node, with timing from the `:CONTAINS` edge and
/// speaker attribution from `:SAYS`.
#[derive(Debug, Serialize)]
pub struct Statement {
    pub text: String,
    /// The person who said this (via `:SAYS`).
    pub person: Person,
    /// Seconds from start of recording. Null for non-media statements.
    pub start_time: Option<f64>,
    pub end_time: Option<f64>,
    /// Per-word timing data as a JSON string, e.g.
    /// `[{"word":"the","start":1.23,"end":1.45}, ...]`.
    /// Null if the transcription pipeline did not produce word-level timing.
    pub words: Option<String>,
}

/// Mirrors the (:Interview) node.
///
/// `date` is stored as a Neo4j Date and deserialized into
/// `chrono::NaiveDate`. `Serialize` emits ISO 8601 strings ("2003-05-05"),
/// which is the expected shape for REST consumers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Interview {
    pub uid: String,
    pub number: i64,
    pub interviewee: String,
    pub date: NaiveDate,
}

/// Mirrors the (:Transcript) node, with its ordered statements and the
/// interview it belongs to.
#[derive(Debug, Serialize)]
pub struct Transcript {
    pub uid: String,
    /// The interview this transcript belongs to (via `:HAS_TRANSCRIPT`).
    pub interview: Interview,
    /// Statements in transcript order (ordered by `startTime` on `:CONTAINS`).
    pub statements: Vec<Statement>,
}

/// A single hit from a vector similarity search over Statement nodes.
#[derive(Debug, Serialize)]
pub struct SearchHit {
    /// The matching statement, with speaker and timing.
    pub statement: Statement,
    /// The interview this statement belongs to.
    pub interview: Interview,
}

// ---------------------------------------------------------------------------
// Asset types
// ---------------------------------------------------------------------------

/// The kind of media asset attached to an interview.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    Video,
    Unknown,
}

/// A media asset node (`:Asset`) in the graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asset {
    pub uid: String,
    pub uri: String,
    pub kind: AssetKind,
}
