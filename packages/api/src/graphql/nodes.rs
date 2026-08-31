// FIXME: This is a junk-drawer module. It groups types by *kind* ("structs that
// deserialize from a Neo4j node") rather than by a shared reason to change:
// `Person` moves when the speaker model moves, `Interview` when the archive
// schema moves. Grouping by kind is a cohesion smell. This module exists only
// so `interviews.rs` stops silently doubling as the shared type hub; it is a
// transitional home, not a destination. Revisit --- likely colocate each
// projection with the domain that owns it, or lift the node projections into
// `auohp-core` alongside the rest of the graph model.

use async_graphql::SimpleObject;
use serde::Deserialize;

// Each struct below mirrors one node label in the Neo4j graph. The mapping is
// deliberately close to the graph model so that schema changes surface as
// compilation errors. Relationships with properties (`:CONTAINS` timing) are
// represented as optional fields on the target type, since GraphQL has no
// native concept of a property-bearing edge.

/// Mirrors the (:Person) node.
///
/// Derives `Deserialize` so neo4rs can deserialize a Bolt Node directly into
/// this struct via `node.to::<Person>()`. The field names (`uid`, `name`)
/// match the Neo4j property names exactly, so no `#[serde(rename)]` is needed.
///
/// This is the idiomatic neo4rs pattern: return whole nodes from Cypher
/// (`RETURN p`) rather than destructuring properties into arbitrary column
/// aliases (`RETURN p.uid AS person_uid, p.name AS person_name`).
#[derive(SimpleObject, Clone, Deserialize)]
pub struct Person {
    pub uid: String,
    pub name: String,
}

/// The node-native properties of a (:Statement) node.
///
/// Separated from `Statement` because `Statement` mixes node properties with
/// data from relationships (`:CONTAINS` timing, `:SAYS` speaker), making a
/// full `Deserialize` derive impossible. This struct covers only what lives on
/// the node itself. `#[serde(default)]` on `words` means serde substitutes
/// `None` for absent keys rather than erroring---older transcripts were seeded
/// without word-level timing, so the property may not exist on the node at all.
#[derive(Deserialize, SimpleObject, Clone, Debug)]
pub struct StatementNode {
    pub uid: String,
    pub text: String,
    #[serde(default)]
    pub words: Option<String>,
}

/// Mirrors the (:Statement) node, with timing from the `:CONTAINS` edge and
/// speaker attribution from `:SAYS`.
#[derive(SimpleObject, Deserialize, Clone)]
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
