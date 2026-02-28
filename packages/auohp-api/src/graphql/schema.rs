use async_graphql::{Context, EmptyMutation, EmptySubscription, Object, Schema};

use crate::neo4j::Db;
use super::interviews::{self, Interview, Transcript};

// AppSchema is a type alias for the fully-parameterized Schema type.
// Schema<Q, M, S> takes three type parameters: query root, mutation root,
// and subscription root. Using EmptyMutation / EmptySubscription is the
// idiomatic way to say "this schema has no mutations or subscriptions yet."
pub type AppSchema = Schema<QueryRoot, EmptyMutation, EmptySubscription>;

pub struct QueryRoot;

// #[Object] generates the GraphQL resolver machinery from the method
// signatures below. Each async method becomes a GraphQL field; its return
// type becomes the field's GraphQL type; doc comments become field
// descriptions in the introspection schema.
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

    /// Returns the full time-ordered transcript for a single interview.
    async fn interview_transcript(
        &self,
        ctx: &Context<'_>,
        number: i64,
    ) -> async_graphql::Result<Transcript> {
        interviews::get_transcript(ctx, number).await
    }
}

// build_schema() accepts the connection pool and embeds it as schema-level
// data. Resolvers retrieve it via ctx.data::<Db>() — async-graphql's
// equivalent of axum's State<T>, but scoped to the GraphQL execution context
// rather than the HTTP layer.
pub fn build_schema(db: Db) -> AppSchema {
    Schema::build(QueryRoot, EmptyMutation, EmptySubscription)
        .data(db)
        .finish()
}
