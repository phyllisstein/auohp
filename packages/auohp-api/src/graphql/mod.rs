use async_graphql::{Context, EmptyMutation, EmptySubscription, Object, Schema};

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
}

pub fn build_schema() -> AppSchema {
    Schema::build(QueryRoot, EmptyMutation, EmptySubscription).finish()
}
