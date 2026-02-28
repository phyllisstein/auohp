use anyhow::Result;
use neo4rs::Graph;
use std::sync::Arc;

// Db is a cloneable, thread-safe handle to the Neo4j connection pool.
//
// Arc<T> (Atomic Reference Counted) is Rust's multi-ownership smart pointer
// for concurrent code. Cloning an Arc<T> increments an atomic reference count
// rather than copying T, so every axum handler that needs the pool gets a
// cheap Arc clone — they all share the same Graph underneath.
//
// Graph itself is the connection pool; neo4rs manages Bolt connection reuse
// internally, so we never create more than one Graph per server process.
pub type Db = Arc<Graph>;

// Opens a Bolt connection pool to Neo4j and wraps it in an Arc.
//
// Graph::new() authenticates immediately on the first connection. If the URI
// is unreachable or credentials are wrong, it returns an error here, before
// the HTTP server ever starts — which is exactly what we want: fail fast at
// startup rather than serving 500s on the first real request.
pub async fn connect(uri: &str, user: &str, password: &str) -> Result<Db> {
    let graph = Graph::new(uri, user, password).await?;
    Ok(Arc::new(graph))
}
