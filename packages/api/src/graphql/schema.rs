use std::sync::Arc;

use async_graphql::{EmptySubscription, Schema};

use super::mutations::MutationRoot;
use super::queries::QueryRoot;
use crate::neo4j::Db;
use auohp_core::embeddings::EmbedderHandle;

pub type AppSchema = Schema<QueryRoot, MutationRoot, EmptySubscription>;

pub fn build_schema(db: Db, embedder: Arc<EmbedderHandle>) -> AppSchema {
    Schema::build(QueryRoot, MutationRoot, EmptySubscription)
        .data(db)
        .data(embedder)
        .finish()
}
