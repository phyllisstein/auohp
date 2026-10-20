use rustls::crypto::{CryptoProvider, aws_lc_rs};
mod captions;
mod graphql;
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
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::{error, info};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

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

    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "auohp_api=debug,auohp_core=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();

    // Docker mounts .env as a secret at this path; local dev uses ./.env.
    dotenvy::from_path("/run/secrets/environment").ok();
    dotenvy::dotenv().ok();

    let neo4j_uri = std::env::var("NEO4J_URI").unwrap_or_else(|_| "neo4j://neo4j:7687".to_string());
    let neo4j_user = std::env::var("NEO4J_USERNAME").unwrap_or_else(|_| "neo4j".to_string());
    let neo4j_password = std::env::var("NEO4J_PASSWORD").unwrap_or_else(|_| "neo4j".to_string());
    let neo4j_database = std::env::var("NEO4J_DATABASE").unwrap_or_else(|_| "neo4j".to_string());

    let db = neo4j::connect(&neo4j_uri, &neo4j_user, &neo4j_password, &neo4j_database).await?;
    info!("connected to Neo4j at {neo4j_uri}");

    db.run(neo4rs::query(
        "CREATE VECTOR INDEX statementEmbedding IF NOT EXISTS
         FOR (s:Statement) ON s.embedding
         OPTIONS {indexConfig: {
           `vector.dimensions`: 1024,
           `vector.similarity_function`: 'cosine',
           `vector.quantization.type`: 'binary',
           `vector.default_search_expansion_factor`: 3.0
         }}",
    ))
    .await?;

    db.run(neo4rs::query(
        "
        CREATE FULLTEXT INDEX statementSearchText IF NOT EXISTS
        FOR (s:Statement) ON EACH [s.text]
        OPTIONS {indexConfig: {
            `fulltext.analyzer`: 'english',
            `fulltext.eventually_consistent`: true
        }}
    ",
    ))
    .await?;
    info!("ensured searchable text Statement index");

    let embedder = Embedder::new().expect("failed to load embedding model");
    info!("loaded embedding model ({}-dim)", &embedder.dimensions());
    let embed_handler = std::sync::Arc::new(EmbedderHandle::new(embedder));

    let captions_db = Arc::clone(&db);
    let schema = graphql::build_schema(db, embed_handler);

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
            // GET serves GraphiQL; POST executes queries.
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
