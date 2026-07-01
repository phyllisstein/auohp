mod error;
mod interviews;
mod mutations;
pub mod queries;
mod schema;
mod search;

// FIXME: Structure of thse modules makes no sense.
pub use schema::{AppSchema, build_schema};
