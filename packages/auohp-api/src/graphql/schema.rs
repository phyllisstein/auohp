use std::sync::Arc;

use async_graphql::{Context, EmptySubscription, Object, Schema};

use crate::embeddings::Embedder;
use crate::neo4j::Db;
use super::interviews::{self, Interview, Transcript};
use super::mutations::MutationRoot;

pub type AppSchema = Schema<QueryRoot, MutationRoot, EmptySubscription>;

pub struct QueryRoot;

#[Object]
impl QueryRoot {
    /// Returns "ok". Useful for readiness and liveness probes.
    async fn health(&self, _ctx: &Context<'_>) -> &'static str {
        "ok"
    }

    /// Lists all interviews in the archive, ordered by date.
    async fn interviews(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<Interview>> {
        interviews::list_interviews(ctx).await
    }

    /// Returns the full transcript for a single interview, statements ordered
    /// by the `:NEXT` linked list in Neo4j.
    async fn interview_transcript(
        &self,
        ctx: &Context<'_>,
        number: i64,
    ) -> async_graphql::Result<Transcript> {
        interviews::get_transcript(ctx, number).await
    }
}

pub fn build_schema(db: Db, embedder: Arc<Embedder>) -> AppSchema {
    Schema::build(QueryRoot, MutationRoot, EmptySubscription)
        .data(db)
        .data(embedder)
        .finish()
}
