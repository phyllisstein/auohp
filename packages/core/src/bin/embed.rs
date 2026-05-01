//! Standalone vector embedding binary for acceptance testing search.
//!
//! Usage:
//!   cargo run --bin embed -- <query> [<query>] [<query>]
//!
//! Writes the embedded vector to stdout; logs go to stderr.

#[path = "../embeddings.rs"]
mod embeddings;

use anyhow::Result;
use clap::Parser;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser, Debug)]
struct Cli {
    /// Strings to embed
    queries: Vec<String>,
}

fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "auohp_core=debug".into()))
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stdout))
        .init();

    let cli = Cli::parse();

    let embedder = if let Ok(embedder) = embeddings::Embedder::new() {
        embedder
    } else {
        panic!("Could not create embedder");
    };

    for query in cli.queries {
        if let Ok(vector) = embedder.embed(std::slice::from_ref(&query)) {
            println!(
                "\n\n:param {query}Embedding => {:?}",
                vector.first().unwrap()
            );
        } else {
            tracing::error!(query, "embedding failed")
        }
    }

    Ok(())
}
