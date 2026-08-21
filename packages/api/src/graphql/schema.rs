use std::sync::Arc;

use async_graphql::{EmptyMutation, EmptySubscription, MergedObject, Schema};

use super::mutations::MutationRoot;
use super::queries::{
    QueryRoot,
    captions::{Captions, CaptionsQuery},
};
use crate::neo4j::Db;
use auohp_core::embeddings::EmbedderHandle;

#[derive(MergedObject, Default)]
pub struct Query(CaptionsQuery);

pub type AppSchema = Schema<Query, EmptyMutation, EmptySubscription>;

pub fn build_schema(db: Db, embedder: Arc<EmbedderHandle>) -> AppSchema {
    Schema::build(Query::default(), EmptyMutation, EmptySubscription)
        .data(db)
        .data(embedder)
        .finish()
}
