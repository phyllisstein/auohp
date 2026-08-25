use super::queries::{captions, interviews};
use crate::neo4j::Db;
use async_graphql::{EmptyMutation, EmptySubscription, MergedObject, Schema};
use auohp_core::embeddings::EmbedderHandle;
use std::sync::Arc;

#[derive(MergedObject, Default)]
pub struct Query(captions::CaptionsQuery, interviews::InterviewQuery);

pub type AppSchema = Schema<Query, EmptyMutation, EmptySubscription>;

pub fn build_schema(db: Db, embedder: Arc<EmbedderHandle>) -> AppSchema {
    Schema::build(Query::default(), EmptyMutation, EmptySubscription)
        .data(db)
        .data(embedder)
        .finish()
}
