use super::mutations::MutationRoot;
use super::queries::{captions, interviews, jobs, root, search};
use crate::jobs::JobQueue;
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
    jobs::JobsQuery,
);

pub type AppSchema = Schema<Query, MutationRoot, EmptySubscription>;

/// Build the executable schema.
///
/// `queue` joins `db` and `embedder` as a third injected dependency. All three
/// arrive the same way --- `.data()` puts a value into a type-keyed map, and
/// `ctx.data::<T>()` pulls it back out by that type. The lookup is dynamic
/// (it is a `TypeId` map under the hood), which is why `JobQueue` is a
/// distinct newtype rather than a bare `SqlitePool`: the type *is* the key, so
/// two dependencies that happened to share a type would collide.
pub fn build_schema(db: Db, embedder: Arc<EmbedderHandle>, queue: JobQueue) -> AppSchema {
    Schema::build(Query::default(), MutationRoot, EmptySubscription)
        .data(db)
        .data(embedder)
        .data(queue)
        .finish()
}
