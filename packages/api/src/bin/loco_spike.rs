//! Loco migration spike --- entrypoint.
//!
//! Compare this against `main.rs`. That file is 155 lines and does everything
//! explicitly: install the crypto provider, init tracing, load dotenv, connect
//! to Neo4j, ensure indexes, load the embedder, build the schema, build the
//! router, bind the listener, serve with graceful shutdown. Every step is
//! visible and ordered.
//!
//! This file is a single call. The steps still happen --- they are just
//! distributed across `Hooks` implementations and Loco's boot sequence, and the
//! order in which they run is a property of the framework rather than of any
//! file you can read.
//!
//! That trade is the whole question the spike exists to pose. It buys a CLI, a
//! config system, a middleware stack and a scheduler. It costs local
//! legibility of startup, and it puts a 116-crate dependency between this
//! service and the two libraries it actually needs (`axum`, `neo4rs`).

use auohp_api::loco_app::App;
use loco_rs::cli;

#[tokio::main]
async fn main() -> loco_rs::Result<()> {
    // dotenv still has to happen manually and *before* Loco boots, because
    // Loco's own config layer reads `LOCO_ENV` from the environment during
    // `Environment::resolve`. Loco has no hook that runs earlier than its own
    // config load, so this cannot move into `Hooks`.
    dotenvy::from_path("/run/secrets/environment").ok();
    dotenvy::dotenv().ok();

    // The `with-db` flavor of this call is `cli::main::<App, Migrator>()`,
    // taking a `sea_orm_migration::MigratorTrait` type parameter. The no-db
    // overload drops it. This single line is where the SeaORM question is
    // actually decided at the type level.
    cli::main::<App>().await
}
