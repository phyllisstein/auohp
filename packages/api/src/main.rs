use rustls::crypto::{CryptoProvider, aws_lc_rs};
mod captions;
mod graphql;
mod jobs;
mod neo4j;
mod uid;
use anyhow::Result;
use async_graphql::http::GraphiQLSource;
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
use auohp_core::embeddings::{Embedder, EmbedderHandle};
use axum::{
    Router,
    extract::{DefaultBodyLimit, Path, State},
    http::header,
    response::{Html, IntoResponse},
    routing::get,
};
use http::StatusCode;
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    trace::TraceLayer,
};
use tracing::{error, info};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

// The GraphQL handler receives two arguments:
//
//   State(schema)  ---axum's dependency-injection mechanism. The schema is
//                     stored in the Router via .with_state() and extracted
//                     here with State<T>. The destructuring syntax
//                     `State(schema)` unwraps the newtype wrapper in one step.
//
//   req            ---the incoming GraphQL request, deserialized from JSON
//                     by async-graphql-axum.
async fn graphql_handler(
    State(schema): State<graphql::AppSchema>,
    req: GraphQLRequest,
) -> GraphQLResponse {
    schema.execute(req.into_inner()).await.into()
}

#[tokio::main]
async fn main() -> Result<()> {
    CryptoProvider::install_default(aws_lc_rs::default_provider())
        .map_err(|_| anyhow::anyhow!("default crypto provider already installed"))?;

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
    // embeddings. IF NOT EXISTS makes this idempotent across restarts.

    db.run(neo4rs::query(
        "CREATE VECTOR INDEX statementEmbedding IF NOT EXISTS
         FOR (s:Statement) ON s.embedding
         OPTIONS {indexConfig: {
           `vector.dimensions`: 768,
           `vector.similarity_function`: 'cosine'
         }}",
    ))
    .await?;

    db.run(neo4rs::query(
        "CREATE FULLTEXT INDEX statementText IF NOT EXISTS
         FOR (s:Statement) ON EACH [s.text]",
    ))
    .await?;
    info!("ensured fulltext Statement index");

    let embedder = Embedder::new().expect("failed to load embedding model");
    info!("loaded embedding model ({}-dim)", &embedder.dimensions());
    let embed_handler = std::sync::Arc::new(EmbedderHandle::new(embedder));

    // ── Background job queue ─────────────────────────────────────────────
    //
    // Opened before the schema is built, because the schema needs a handle to
    // inject. `open_pool` creates the database file and its parent directory
    // if missing and runs apalis's migrations, all idempotently --- there is
    // no separate install step, and a fresh checkout boots without ceremony.
    let jobs_config = jobs::StorageConfig::default();
    let jobs_pool = jobs::open_pool(&jobs_config).await?;
    info!(url = %jobs_config.url, "opened background job queue");

    let queue = jobs::JobQueue::new(jobs_pool.clone(), &jobs_config);

    // The worker's dependencies. Both are the same handles the resolvers hold:
    // `Db` is an `Arc<Graph>` (a connection pool) and `EmbedderHandle` is an
    // `Arc` around channel senders. Cloning either is a refcount bump, so the
    // worker and the HTTP server genuinely share one Neo4j pool and one ONNX
    // thread rather than standing up duplicates.
    let worker_deps = jobs::embed::EmbedDeps {
        db: Arc::clone(&db),
        embedder: Arc::clone(&embed_handler),
    };

    // One shutdown signal, two consumers. `tokio::sync::watch` is the right
    // primitive here rather than a oneshot: a oneshot can only be awaited
    // once, and both axum and the worker need to observe the same edge.
    // Awaiting `changed()` on a receiver is the "wait for the flag to flip"
    // half; the sender is fired from `shutdown_signal` below.
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(false);

    let worker_shutdown = {
        let mut rx = shutdown_rx.clone();
        async move {
            // `changed()` errors only if every sender was dropped, which also
            // means shutdown --- so either arm of this is "stop".
            let _ = rx.changed().await;
        }
    };

    // Spawn the worker. It polls the SQLite queue, so anything left `Pending`
    // from a previous run is picked up here --- this line is where restart
    // recovery actually happens.
    let worker_handle = tokio::spawn(jobs::queue::run_worker(
        jobs_pool.clone(),
        jobs_config.clone(),
        worker_deps,
        worker_shutdown,
    ));
    info!("background embedding worker started");

    let captions_db = Arc::clone(&db);
    let schema = graphql::build_schema(db, embed_handler, queue);

    let app =
        Router::new()
            .route("/health", get(|| async { "ok" }))
            .route(
                "/interview/{interview_number}/vtt",
                get(async move |Path(interview_number): Path<i64>| {
                    match crate::captions::generate_vtt(&captions_db, interview_number).await {
                        Ok(vtt) => ([(header::CONTENT_TYPE, "text/vtt")], vtt).into_response(),
                        Err(e) => {
                            error!(interview_number, error = %e, "failed to generate captions");
                            StatusCode::INTERNAL_SERVER_ERROR.into_response()
                        }
                    }
                }),
            )
            // GET  /graphql --> serves the GraphiQL interactive IDE, so you can
            //                  explore the schema and test queries from a browser.
            // POST /graphql --> the actual GraphQL execution endpoint.
            //
            // GraphiQLSource generates a self-contained HTML page that talks to
            // the POST endpoint. It's baked into async-graphql behind the
            // "graphiql" feature flag.
            .route(
                "/graphql",
                get(|| async {
                    Html(
                        GraphiQLSource::build()
                            .endpoint("/graphql")
                            .title("AUOHP GraphQL")
                            .finish(),
                    )
                })
                .post(graphql_handler),
            )
            .layer(
                ServiceBuilder::new()
                    .layer(TraceLayer::new_for_http())
                    .layer(DefaultBodyLimit::max(16 * 1024 * 1024))
                    .layer(CorsLayer::permissive()),
            )
            // with_state() makes `schema` available to any handler that
            // declares a State<AppSchema> parameter.
            .with_state(schema);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:6060").await?;
    info!("listening on {}", listener.local_addr()?);

    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            shutdown_signal().await;
            // Tell the worker to stop too. `send` fails only if every receiver
            // was dropped, which would mean the worker already exited --- so a
            // failure here is not worth reporting.
            let _ = shutdown_tx.send(true);
        })
        .await?;

    // Wait for the worker to finish draining before the process exits.
    //
    // Ordering matters and is the whole point of joining here rather than
    // letting the runtime drop the task: a task still executing when `main`
    // returns is aborted mid-flight. Awaiting the handle lets an in-progress
    // embedding either finish or --- if the process is killed harder --- leave
    // its row in `Running` for the orphan reaper to reclaim on the next boot.
    //
    // Note the double `?`-shaped unwrapping: `JoinHandle` yields
    // `Result<T, JoinError>` (did the task panic?) wrapping the task's own
    // `anyhow::Result<()>` (did the work fail?). Two independent failure modes,
    // two layers.
    match worker_handle.await {
        Ok(Ok(())) => info!("background embedding worker stopped cleanly"),
        Ok(Err(e)) => error!(error = %e, "background embedding worker failed"),
        Err(e) => error!(error = %e, "background embedding worker panicked"),
    }

    // Silence the unused-variable warning on the retained receiver; it exists
    // so the watch channel keeps at least one receiver alive for the worker's
    // clone to observe.
    let _ = shutdown_rx.borrow_and_update();

    Ok(())
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install Ctrl+C signal handler");
}
