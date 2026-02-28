use anyhow::Result;
use axum::{routing::get, Router};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize structured logging. The RUST_LOG environment variable controls
    // which log levels are emitted (e.g. RUST_LOG=debug). If unset, default to
    // debug-level output from this crate only.
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "auohp_api=debug".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load a .env file if one is present; silently skip if not. The .ok()
    // call discards the Result — in production there is no .env file, and
    // that's fine.
    dotenvy::dotenv().ok();

    let app = Router::new().route("/health", get(|| async { "ok" }));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:6060").await?;
    info!("listening on {}", listener.local_addr()?);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

// Waits for Ctrl+C before returning, which signals axum to stop accepting
// new connections and let in-flight requests drain.
async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install Ctrl+C signal handler");
}
