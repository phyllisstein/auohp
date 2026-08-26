use super::mutations::MutationRoot;
use super::queries::{QueryRoot, captions, interviews};
use crate::neo4j::Db;
use async_graphql::{EmptySubscription, MergedObject, Schema};
use auohp_core::embeddings::EmbedderHandle;
use std::sync::Arc;

#[derive(MergedObject, Default)]
pub struct Query(
    captions::CaptionsQuery,
    interviews::InterviewQuery,
    QueryRoot,
);

pub type AppSchema = Schema<Query, MutationRoot, EmptySubscription>;

pub fn build_schema(db: Db, embedder: Arc<EmbedderHandle>) -> AppSchema {
    Schema::build(Query::default(), MutationRoot, EmptySubscription)
        .data(db)
        .data(embedder)
        .finish()
}
