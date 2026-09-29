//! Standalone vector embedding binary for acceptance testing search.
//!
//! Usage:
//!   cargo run --bin embed -- <query> [<query>] [<query>]
//!
//! Writes the embedded vector to stdout; logs go to stderr.

use anyhow::Result;
use auohp_core::embeddings::Embedder;
use clap::Parser;
use cruet::Inflector;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Parser, Debug)]
struct Cli {
    /// Strings to embed
    queries: Vec<String>,
}

fn truncated_camel_case(other_cased: &str) -> String {
    other_cased
        .unicode_words()
        .take(3)
        .collect::<Vec<&str>>()
        .join(" ")
        .to_camel_case()
}

fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "auohp_core=debug".into()))
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stdout))
        .init();

    let cli = Cli::parse();

    let mut embedder = embeddings::Embedder::new()?;

    for query in cli.queries {
        let vector = embedder.embed(std::slice::from_ref(&query))?;
        let camel = truncated_camel_case(&query);
        println!(
            "\n\n:param {camel}Embedding => {:?}",
            vector.first().unwrap()
        );
    }

    Ok(())
}
