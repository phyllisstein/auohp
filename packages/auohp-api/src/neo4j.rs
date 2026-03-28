use anyhow::{Context, Result};
use neo4rs::{ConfigBuilder, Graph};
use std::sync::Arc;
use std::time::Duration;

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
// Graph::new() builds the pool configuration but is lazy — it does not open
// a socket until the first query runs. We follow it with a no-op ping query
// so that unreachable hosts or bad credentials fail here, before the HTTP
// server starts, rather than on the first real request.
pub async fn connect(uri: &str, user: &str, password: &str, database: &str) -> Result<Db> {
    let config = ConfigBuilder::default()
        .uri(uri)
        .user(user)
        .password(password)
        .db(database)
        .build()
        .unwrap();
    let graph = Graph::connect(config)?;
    tokio::time::timeout(Duration::from_secs(5), graph.run(neo4rs::query("RETURN 1")))
        .await
        .context("timed out connecting to Neo4j")??;
    Ok(Arc::new(graph))
}
