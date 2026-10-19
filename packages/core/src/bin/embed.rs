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

    for query in cli.queries {
        let vector = embedder.embed(std::slice::from_ref(&query))?;
        let vector: Vec<f64> = vector
            .into_iter()
            .flat_map(|v| v.into_iter().map(|q| f64::from(q)))
            .collect();

        let camel = truncated_camel_case(&query);
        println!(
            "\n:param {}Embedding => vector({:?}, 1024, FLOAT64);\n",
            camel,
            vector.first().unwrap()
        );
    }

    Ok(())
}
