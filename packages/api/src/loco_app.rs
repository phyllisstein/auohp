//! Loco migration spike --- the AUOHP API expressed as a `loco_rs::app::Hooks`
//! implementation.
//!
//! This is exploratory. It exists to answer one question in code rather than in
//! prose: what does this service look like once Loco owns the process lifecycle,
//! given that the persistence layer is Neo4j over Bolt and Loco's own
//! persistence story is SeaORM over SQL?
//!
//! The short answer is that it works, but only because every Loco feature that
//! touches SQL is switched off at the Cargo level (`default-features = false`,
//! `features = ["cli"]`). What survives is Loco's boot sequence, its config
//! loader, its middleware stack, and its CLI. See `docs/spikes/loco-migration.md`
//! for the accounting.
//!
//! ## The three mechanisms doing the real work here
//!
//! 1. `AppContext.db` is `#[cfg(feature = "with-db")]`. With the feature off,
//!    the field does not exist and `AppContext::builder` loses its
//!    `DatabaseConnection` argument. The no-SQL build is a first-class,
//!    compiled-in configuration --- not a workaround.
//!
//! 2. `AppContext.shared_store` is a `DashMap<TypeId, Box<dyn Any + Send + Sync>>`.
//!    That is the same type-keyed heterogeneous map trick async-graphql's
//!    `.data()` / `ctx.data::<T>()` uses, and the same one `http::Extensions`
//!    uses. It is where `Arc<Graph>`, `Arc<EmbedderHandle>` and the built
//!    schema live, because Loco gives us no typed slot for them.
//!
//! 3. `AppRoutes::to_router` ends in `app.with_state(ctx)`, so the Axum state
//!    type is *fixed* to `AppContext`. The existing `.with_state(schema)` in
//!    `main.rs` cannot be carried over --- handlers must reach the schema
//!    through `AppContext` instead. This is the single largest shape change the
//!    migration forces on existing code.

use std::sync::Arc;

use async_graphql::http::GraphiQLSource;
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
use auohp_core::embeddings::{Embedder, EmbedderHandle};
use axum::{
    Router as AxumRouter,
    extract::{Path, State},
    http::header,
    response::{Html, IntoResponse},
};
use http::StatusCode;
use loco_rs::{
    app::{AppContext, Hooks, Initializer},
    bgworker::Queue,
    boot::{BootResult, StartMode, create_app},
    config::Config,
    controller::AppRoutes,
    environment::Environment,
    task::Tasks,
    Result as LocoResult,
};
use tracing::{error, info};

use crate::graphql::{self, AppSchema};
use crate::neo4j::{self, Db};

/// The application type. Loco's `Hooks` trait is implemented on a unit struct
/// rather than on a value --- every method is an associated function, so the
/// framework never needs an instance. `create_app::<App>` monomorphizes the
/// whole boot sequence against this type.
pub struct App;

// ---------------------------------------------------------------------------
// Startup work that has no Loco-shaped home
// ---------------------------------------------------------------------------

/// Installs the process-wide rustls crypto provider.
///
/// In `main.rs` this is the first statement in `main`, before anything else can
/// touch TLS. Loco owns `main` now, so the earliest hook available to us is
/// `Hooks::boot`, which runs *after* Loco has already parsed config. That is
/// still early enough in practice --- nothing in Loco's config loading opens a
/// TLS connection --- but the ordering guarantee is weaker than it was, and it
/// is a guarantee we used to get for free from controlling `main`.
///
/// `install_default` returns `Err` if a provider is already installed, which is
/// benign here (repeated boots in tests), so it is deliberately swallowed.
fn install_crypto_provider() {
    let _ = rustls::crypto::CryptoProvider::install_default(
        rustls::crypto::aws_lc_rs::default_provider(),
    );
}

/// Reads Neo4j connection parameters from the environment.
///
/// Note what this function is *not*: it is not reading from Loco's `Config`.
/// Loco's config is a typed struct with a fixed set of fields --- `logger`,
/// `server`, `queue`, `workers`, `mailer`, and (behind `with-db`) `database`.
/// There is no extension point for a Neo4j URI, so the Bolt connection details
/// stay in `std::env` exactly as they are today, sitting alongside a YAML
/// config system that does not know about them. Two configuration mechanisms
/// where there was one.
fn neo4j_params() -> (String, String, String, String) {
    (
        std::env::var("NEO4J_URI").unwrap_or_else(|_| "neo4j://neo4j:7687".to_string()),
        std::env::var("NEO4J_USERNAME").unwrap_or_else(|_| "neo4j".to_string()),
        std::env::var("NEO4J_PASSWORD").unwrap_or_else(|_| "neo4j".to_string()),
        std::env::var("NEO4J_DATABASE").unwrap_or_else(|_| "neo4j".to_string()),
    )
}

/// Ensures the vector and fulltext indexes exist.
///
/// Lifted verbatim from `main.rs`. In the current architecture this runs between
/// "pool is up" and "server is listening", in one linear function, and the
/// ordering is evident from reading top to bottom. Under Loco it runs inside an
/// `Initializer::before_run`, and the ordering is a property of Loco's boot
/// sequence rather than of this file.
async fn ensure_indexes(db: &Db) -> anyhow::Result<()> {
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

    info!("ensured Statement vector and fulltext indexes");
    Ok(())
}

// ---------------------------------------------------------------------------
// The initializer: where the graph, the embedder and the schema get built
// ---------------------------------------------------------------------------

/// Builds every stateful resource the API needs and parks it on the shared store.
///
/// Loco's `Initializer` trait is the closest thing the framework has to "run
/// this during boot and hand the result to the app". It is a reasonable fit ---
/// but note that `before_run` returns `Result<()>`, not `Result<Something>`.
/// There is no channel by which an initializer *returns* a value. The only way
/// to get the graph handle out is to mutate the context's shared store as a
/// side effect, which is why this function ends in three `insert` calls rather
/// than in a return.
pub struct GraphInitializer;

#[async_trait::async_trait]
impl Initializer for GraphInitializer {
    fn name(&self) -> String {
        "auohp-graph".to_string()
    }

    async fn before_run(&self, ctx: &AppContext) -> LocoResult<()> {
        install_crypto_provider();

        let (uri, user, password, database) = neo4j_params();

        // `neo4j::connect` already carries the exponential-backoff retry for the
        // Docker startup race. It survives the migration untouched, because it
        // never knew anything about the web layer to begin with --- which is
        // itself evidence for how little of this codebase is framework-shaped.
        let db = neo4j::connect(&uri, &user, &password, &database)
            .await
            .map_err(|e| loco_rs::Error::Message(format!("neo4j connect: {e}")))?;

        info!("connected to Neo4j at {uri}");

        ensure_indexes(&db)
            .await
            .map_err(|e| loco_rs::Error::Message(format!("index setup: {e}")))?;

        let embedder =
            Embedder::new().map_err(|e| loco_rs::Error::Message(format!("embedder: {e}")))?;
        info!("loaded embedding model ({}-dim)", embedder.dimensions());
        let embedder = Arc::new(EmbedderHandle::new(embedder));

        // The schema is built here and stored whole, rather than assembled per
        // request. async-graphql's `.data()` is its own DI mechanism, entirely
        // independent of Loco's --- so the graph handle ends up stored twice, in
        // two different type-keyed maps, for two different consumers: once inside
        // the schema for resolvers, once on the shared store for the plain-axum
        // VTT handler.
        let schema = graphql::build_schema(Arc::clone(&db), Arc::clone(&embedder));

        ctx.shared_store.insert(db);
        ctx.shared_store.insert(embedder);
        ctx.shared_store.insert(schema);

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// Pulls the prebuilt schema off the context.
///
/// Every handler pays this lookup: a `TypeId` hash into a `DashMap`, a downcast,
/// and a clone of the `Schema` (which is internally `Arc`-ed, so the clone is
/// cheap). Compare with what it replaces --- `State(schema): State<AppSchema>`,
/// which axum resolves at compile time with no runtime lookup at all.
///
/// The `expect` is the honest cost of the shared store: `insert` is not
/// type-checked against `get`, so a missing initializer is a runtime panic where
/// the current design would be a compile error.
fn schema_from(ctx: &AppContext) -> AppSchema {
    ctx.shared_store
        .get_ref::<AppSchema>()
        .expect("GraphQL schema missing from shared store --- GraphInitializer did not run")
        .clone()
}

fn db_from(ctx: &AppContext) -> Db {
    ctx.shared_store
        .get_ref::<Db>()
        .expect("Neo4j handle missing from shared store --- GraphInitializer did not run")
        .clone()
}

/// The GraphQL execution endpoint.
///
/// Note the signature change forced by `to_router`'s `with_state(ctx)`: the
/// extractor is `State<AppContext>`, not `State<AppSchema>`. The schema is
/// recovered inside the body instead of being injected by the framework.
async fn graphql_handler(State(ctx): State<AppContext>, req: GraphQLRequest) -> GraphQLResponse {
    schema_from(&ctx).execute(req.into_inner()).await.into()
}

/// GraphiQL, unchanged from `main.rs`.
async fn graphiql() -> impl IntoResponse {
    Html(
        GraphiQLSource::build()
            .endpoint("/graphql")
            .title("AUOHP GraphQL")
            .finish(),
    )
}

/// WebVTT captions for one interview.
async fn vtt_handler(
    State(ctx): State<AppContext>,
    Path(interview_number): Path<i64>,
) -> impl IntoResponse {
    let db = db_from(&ctx);

    match crate::captions::generate_vtt(&db, interview_number).await {
        Ok(vtt) => ([(header::CONTENT_TYPE, "text/vtt")], vtt).into_response(),
        Err(e) => {
            error!(interview_number, error = %e, "failed to generate captions");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn health() -> impl IntoResponse {
    "ok"
}

// ---------------------------------------------------------------------------
// Hooks
// ---------------------------------------------------------------------------

#[async_trait::async_trait]
impl Hooks for App {
    fn app_name() -> &'static str {
        env!("CARGO_CRATE_NAME")
    }

    fn app_version() -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    /// The no-DB flavor of `create_app`. With `with-db` enabled this would be
    /// `create_app::<Self, Migrator>` and would require a `sea_orm_migration`
    /// migrator type --- a thing this project has no possible way to supply,
    /// because there is no SQL schema to migrate. The `#[cfg(not(feature =
    /// "with-db"))]` overload is what makes the whole spike possible.
    async fn boot(
        mode: StartMode,
        environment: &Environment,
        config: Config,
    ) -> LocoResult<BootResult> {
        create_app::<Self>(mode, environment, config).await
    }

    async fn initializers(_ctx: &AppContext) -> LocoResult<Vec<Box<dyn Initializer>>> {
        Ok(vec![Box::new(GraphInitializer)])
    }

    /// Routing.
    ///
    /// `Routes::add` takes a bare `axum::routing::MethodRouter<AppContext>`, so
    /// async-graphql's handlers drop straight in. Loco's controller model is
    /// *not* REST-shaped in any way that obstructs this --- the "controller"
    /// vocabulary is convention, not constraint. This was the compatibility
    /// question most likely to be a blocker, and it is not one.
    fn routes(_ctx: &AppContext) -> AppRoutes {
        AppRoutes::empty()
            .add_route(loco_rs::controller::Routes::new().add("/health", axum::routing::get(health)))
            .add_route(
                loco_rs::controller::Routes::new()
                    .add("/interview/{interview_number}/vtt", axum::routing::get(vtt_handler)),
            )
            .add_route(
                loco_rs::controller::Routes::new().add(
                    "/graphql",
                    axum::routing::get(graphiql).post(graphql_handler),
                ),
            )
    }

    /// The body limit and CORS layers from `main.rs`.
    ///
    /// Loco has its own configurable middleware stack (`middlewares()`), driven
    /// by YAML --- so CORS and body limits could instead be expressed in
    /// `config/development.yaml`. Doing it here keeps the diff legible and makes
    /// the point that `after_routes` is a plain `AxumRouter -> AxumRouter`
    /// escape hatch. Anything Loco's config cannot express goes here.
    async fn after_routes(router: AxumRouter, _ctx: &AppContext) -> LocoResult<AxumRouter> {
        Ok(router
            .layer(axum::extract::DefaultBodyLimit::max(16 * 1024 * 1024))
            .layer(tower_http::cors::CorsLayer::permissive()))
    }

    /// Required by the trait, and inert.
    ///
    /// There are no background workers, and with `worker` off there is no queue
    /// provider to register them against. The method must still be implemented:
    /// `connect_workers` has no default body. This is the shape of the tax ---
    /// not large, but the trait's surface assumes a fuller application than this
    /// one is.
    async fn connect_workers(_ctx: &AppContext, _queue: &Queue) -> LocoResult<()> {
        Ok(())
    }

    /// Also required, also inert. `truncate`, `seed` and `dump` are all behind
    /// `#[cfg(feature = "with-db")]`, so they vanish entirely --- the one place
    /// where switching off SQL genuinely reduces the trait surface instead of
    /// leaving an empty impl behind.
    fn register_tasks(_tasks: &mut Tasks) {}
}
