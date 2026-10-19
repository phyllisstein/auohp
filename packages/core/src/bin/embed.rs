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
use unicode_segmentation::UnicodeSegmentation;

#[derive(Parser, Debug)]
struct Cli {
    /// Strings to embed
    #[arg(id = "query", long, short)]
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
    let cli = Cli::parse();

    let mut embedder = Embedder::new()?;

    for query in cli.queries.chunks(25) {
        let vector = embedder.embed(query)?;
        let vector: Vec<String> = vector
            .iter()
            .zip(query)
            .map(|a| {
                let n: Vec<f64> = a.0.iter().map(|f| *f as f64).collect();
                let k = truncated_camel_case(a.1);
                format!(":param {}Embedding => vector({:?}, 1024, FLOAT64);", k, n)
            })
            .collect();

        let o = vector.join(&"\n\n");
        println!("{}", o);
    }

    Ok(())
}
