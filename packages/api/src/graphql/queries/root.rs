use async_graphql::Object;

#[derive(Default)]
pub struct QueryRoot;

#[Object]
impl QueryRoot {
    /// Returns "ok". Useful for readiness and liveness probes.
    async fn health(&self) -> &'static str {
        "ok"
    }
}
