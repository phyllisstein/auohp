use super::mutations::MutationRoot;
use super::queries::{captions, interviews, root, search};
use crate::neo4j::Db;
use async_graphql::{EmptySubscription, MergedObject, Schema};
use auohp_core::embeddings::EmbedderHandle;
use std::sync::Arc;

#[derive(MergedObject, Default)]
pub struct Query(
    root::QueryRoot,
    captions::CaptionsQuery,
    interviews::InterviewQuery,
    search::SearchQuery,
);

pub type AppSchema = Schema<Query, MutationRoot, EmptySubscription>;

pub fn build_schema(db: Db, embedder: Arc<EmbedderHandle>) -> AppSchema {
    Schema::build(Query::default(), MutationRoot, EmptySubscription)
        .data(db)
        .data(embedder)
        .finish()
}
