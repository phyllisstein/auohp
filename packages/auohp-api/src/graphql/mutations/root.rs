use async_graphql::{Context, Object};
use super::seed_interview::{self, SeedInterviewInput, SeedInterviewPayload};

pub struct MutationRoot;

#[Object]
impl MutationRoot {
    /// Seeds a complete interview: creates Interview, Person, Transcript, and
    /// Statement nodes plus all relationships. Segments are grouped into
    /// statements by consecutive same-speaker runs.
    async fn seed_interview(
        &self,
        ctx: &Context<'_>,
        input: SeedInterviewInput,
    ) -> async_graphql::Result<SeedInterviewPayload> {
        seed_interview::seed_interview(ctx, input).await
    }
}
