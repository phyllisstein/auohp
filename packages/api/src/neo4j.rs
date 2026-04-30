use anyhow::{Context, Result};
use neo4rs::{ConfigBuilder, Graph};
use std::sync::Arc;
use tokio_retry::{Retry, strategy::ExponentialBackoff};

// Db is a cloneable, thread-safe handle to the Neo4j connection pool.
//
// Arc<T> (Atomic Reference Counted) is Rust's multi-ownership smart pointer
// for concurrent code. Cloning an Arc<T> increments an atomic reference count
// rather than copying T, so every axum handler that needs the pool gets a
// cheap Arc clone---they all share the same Graph underneath.
//
// Graph itself is the connection pool; neo4rs manages Bolt connection reuse
// internally, so we never create more than one Graph per server process.
pub type Db = Arc<Graph>;

// Opens a Bolt connection pool to Neo4j and wraps it in an Arc.
//
// Graph::connect() builds the pool configuration but is lazy---it does not
// open a socket until the first query runs. We follow it with a ping query
// so that unreachable hosts or bad credentials fail here, before the HTTP
// server starts, rather than on the first real request.
//
// The ping is retried with exponential backoff (500ms → 1s → 2s → ...) up to
// 10 attempts. This handles the Docker startup race where the Neo4j container
// is reachable by hostname but hasn't yet opened its Bolt port.
pub async fn connect(uri: &str, user: &str, password: &str, database: &str) -> Result<Db> {
    let config = ConfigBuilder::default()
        .uri(uri)
        .user(user)
        .password(password)
        .db(database)
        .build()
        .unwrap();

    let graph = Arc::new(Graph::connect(config)?);

    // ExponentialBackoff::from_millis(500) produces a strategy iterator that
    // yields 500ms, 1000ms, 2000ms, ... delays between attempts. .take(10)
    // caps it at 10 total attempts (~8.5 minutes worst-case ceiling).
    //
    // Retry::spawn re-invokes the closure on each failure; it only stops when
    // the closure returns Ok or the strategy iterator is exhausted.
    let strategy = ExponentialBackoff::from_millis(500).take(10);
    let mut attempt = 0usize;

    Retry::spawn(strategy, || {
        attempt += 1;
        let attempt = attempt;
        let graph = Arc::clone(&graph);

        async move {
            tracing::info!(uri, attempt, "connecting to neo4j");

            graph.run(neo4rs::query("RETURN 1")).await.map_err(|e| {
                tracing::warn!(error = e.to_string(), "connection failed");

                anyhow::anyhow!(e)
            })
        }
    })
    .await
    .context("failed to connect to Neo4j")?;

    tracing::info!(attempt, uri, "connection succeeded");

    Ok(graph)
}
