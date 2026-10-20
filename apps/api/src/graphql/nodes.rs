// FIXME: junk drawer grouped by kind; colocate each projection with its domain
// or lift them into `auohp-core`.

use async_graphql::SimpleObject;
use serde::Deserialize;

// Relationship properties (`:CONTAINS` timing) become optional fields on the
// target type, since GraphQL has no property-bearing edges.

/// Mirrors the (:Person) node.
#[derive(SimpleObject, Clone, Deserialize, Debug)]
pub struct Person {
    pub uid: String,
    pub name: String,
}

/// The node-native properties of a (:Statement) node.
///
/// Split from `Statement`, which also carries relationship data (`:CONTAINS`
/// timing, `:SAYS` speaker) and so cannot derive `Deserialize` from the node.
/// `words` defaults to `None` because older transcripts were seeded without
/// word-level timing.
#[derive(Deserialize, SimpleObject, Clone, Debug)]
pub struct StatementNode {
    pub uid: String,
    pub text: String,
    #[serde(default)]
    pub words: Option<String>,
}

/// Mirrors the (:Statement) node, with timing from the `:CONTAINS` edge and
/// speaker attribution from `:SAYS`.
#[derive(SimpleObject, Deserialize, Clone, Debug)]
pub struct Statement {
    pub uid: String,
    pub text: String,
    /// The person who said this (via `:SAYS`).
    pub person: Option<Person>,
    /// Seconds from start of recording. Null for non-media statements
    /// (e.g. broadsheet text).
    pub start_time: Option<f64>,
    pub end_time: Option<f64>,
    /// Per-word timing data as a JSON string, e.g.
    /// `[{"word":"the","start":1.23,"end":1.45}, ...]`.
    /// Null if the transcription pipeline did not produce word-level timing.
    pub words: Option<String>,
}

// FIXME: Transcript as a full-fledged Object should fetch statements lazily, in
// a separate resolver.
#[derive(SimpleObject, Clone, Deserialize)]
pub struct Transcript {
    pub uid: String,
    /// Statements in transcript order (via the `:NEXT` linked list).
    pub statements: Vec<Statement>,
}

#[derive(SimpleObject, Deserialize)]
pub struct Video {
    pub uri: String,
}
