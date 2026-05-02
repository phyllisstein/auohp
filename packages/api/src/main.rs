mod error;
mod handlers;
mod models;
mod neo4j;

use std::sync::Arc;

use anyhow::Result;
use auohp_core::embeddings::EmbedderHandle;
use axum::{
    Router,
    routing::{get, post},
};
use tracing::info;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

// ---------------------------------------------------------------------------
// Shared application state
// ---------------------------------------------------------------------------

// `AppState` is cloned into every request handler via axum's `State<T>`
// extractor. Both fields are `Arc`-wrapped so that cloning the state gives
// each handler a cheap reference to the same underlying resource --- no
// allocation, no copying, just an atomic counter bump.
//
// `#[derive(Clone)]` on a struct that contains only `Arc<_>` fields is
// idiomatic Rust: it clones the arcs (increments refcounts), not the
// resources behind them.
#[derive(Clone)]
pub struct AppState {
    /// Neo4j connection pool. `Arc<Graph>` is cheaply cloneable; the pool
    /// itself is managed inside `Graph`.
    pub db: neo4j::Db,
    /// Embedding model handle. The inner `Embedder` is wrapped in a `Mutex`
    /// because the ONNX session is not `Send + Sync`. `Arc` lets all handlers
    /// share a single model instance.
    pub embedder: Arc<EmbedderHandle>,
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<()> {
    // Tracing goes to stderr so structured logs don't mix with any stdout
    // output (e.g. health-check scripts that parse the server's stdout).
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "auohp_api=debug".into()))
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();

    // In Docker the .env file is mounted as a secret at this path.
    // In local dev, fall back to a .env file in the working directory.
    dotenvy::from_path("/run/secrets/environment").ok();
    dotenvy::dotenv().ok();

    // Read connection parameters from the environment, with the same defaults
    // used by the TypeScript packages in this monorepo.
    let neo4j_uri = std::env::var("NEO4J_URI").unwrap_or_else(|_| "neo4j://neo4j:7687".to_string());
    let neo4j_user = std::env::var("NEO4J_USERNAME").unwrap_or_else(|_| "neo4j".to_string());
    let neo4j_password = std::env::var("NEO4J_PASSWORD").unwrap_or_else(|_| "neo4j".to_string());
    let neo4j_database = std::env::var("NEO4J_DATABASE").unwrap_or_else(|_| "neo4j".to_string());

    let db = neo4j::connect(&neo4j_uri, &neo4j_user, &neo4j_password, &neo4j_database).await?;
    info!("connected to Neo4j at {neo4j_uri}");

    // Ensure the vector index exists for semantic search over Statement
    // embeddings. IF NOT EXISTS (implicit in CREATE VECTOR INDEX ... IF NOT
    // EXISTS) makes this idempotent across restarts.
    db.run(neo4rs::query(
        "CREATE VECTOR INDEX statement_embedding IF NOT EXISTS
         FOR (s:Statement) ON s.embedding
         OPTIONS {indexConfig: {
           `vector.dimensions`: 768,
           `vector.similarity_function`: 'cosine'
         }}",
    ))
    .await?;
    info!("ensured statement_embedding vector index (768-dim, cosine)");

    let embedder = auohp_core::embeddings::Embedder::new().expect("failed to load embedding model");
    info!("loaded embedding model ({}-dim)", &embedder.dimensions());
    let embed_handler = Arc::new(EmbedderHandle::new(embedder));

    let state = AppState {
        db,
        embedder: embed_handler,
    };

    // ── Route table ─────────────────────────────────────────────────────────
    //
    // Routes are grouped by resource. Each handler module owns its own
    // request/response types; `AppState` is injected via `with_state`.

    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        // Interviews
        .route("/interviews", get(handlers::interviews::list_interviews))
        .route("/interviews", post(handlers::interviews::seed_interview))
        .route(
            "/interviews/{number}",
            get(handlers::interviews::get_transcript),
        )
        // Assets
        .route(
            "/interviews/{number}/assets",
            post(handlers::assets::add_asset),
        )
        // Captions
        .route(
            "/interviews/{number}/captions",
            get(handlers::captions::get_captions),
        )
        // Search
        .route("/search", get(handlers::search::search_statements))
        // with_state makes `state` available to any handler that declares
        // `State<AppState>` as a parameter.
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:6060").await?;
    info!("listening on {}", listener.local_addr()?);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install Ctrl+C signal handler");
}
