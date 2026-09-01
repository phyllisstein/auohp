use super::mutations::MutationRoot;
use super::queries::{captions, interviews, jobs as jobs_query, root, search};
use crate::jobs::Queue;
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
    jobs_query::JobsQuery,
);

pub type AppSchema = Schema<Query, MutationRoot, EmptySubscription>;

/// Build the schema, injecting every shared dependency.
///
/// `.data()` is async-graphql's dependency injection: each value is stored in
/// the schema by its `TypeId` and recovered in a resolver with
/// `ctx.data::<T>()`. The lookup is by type, which is why `Queue` had to be a
/// distinct newtype rather than a bare `Arc<dyn QueueBackend>` --- two
/// dependencies sharing a type would collide, with the later `.data()` call
/// silently overwriting the earlier.
pub fn build_schema(db: Db, embedder: Arc<EmbedderHandle>, queue: Queue) -> AppSchema {
    Schema::build(Query::default(), MutationRoot, EmptySubscription)
        .data(db)
        .data(embedder)
        .data(queue)
        .finish()
}
