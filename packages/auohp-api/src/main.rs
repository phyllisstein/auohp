mod graphql;
mod neo4j;
mod transcription;

use anyhow::Result;
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
use axum::{
    extract::State,
    routing::{get, post},
    Router,
};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

// The GraphQL handler receives two arguments:
//
//   State(schema)   — axum's dependency-injection mechanism. The schema is
//                     stored in the Router via .with_state() and extracted
//                     here with State<T>. The destructuring syntax
//                     `State(schema)` unwraps the newtype wrapper in one step.
//
//   req             — the incoming GraphQL request, deserialized from JSON
//                     by async-graphql-axum.
async fn graphql_handler(
    State(schema): State<graphql::AppSchema>,
    req: GraphQLRequest,
) -> GraphQLResponse {
    schema.execute(req.into_inner()).await.into()
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "auohp_api=debug".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    dotenvy::dotenv().ok();

    // Read connection parameters from the environment, with the same defaults
    // used by the TypeScript packages in this monorepo.
    let neo4j_uri =
        std::env::var("NEO4J_URI").unwrap_or_else(|_| "neo4j://neo4j:7687".to_string());
    let neo4j_user =
        std::env::var("NEO4J_USER").unwrap_or_else(|_| "neo4j".to_string());
    let neo4j_password =
        std::env::var("NEO4J_PASSWORD").unwrap_or_else(|_| "auohpauohp".to_string());

    let db = neo4j::connect(&neo4j_uri, &neo4j_user, &neo4j_password).await?;
    info!("connected to Neo4j at {neo4j_uri}");

    let schema = graphql::build_schema(db);

    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/graphql", post(graphql_handler))
        // with_state() makes `schema` available to any handler that
        // declares a State<AppSchema> parameter.
        .with_state(schema);

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
