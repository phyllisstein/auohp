use super::add_asset::{self, AddAssetInput, AddAssetPayload};
use super::edit_statement::{self, EditStatementInput, EditStatementPayload};
use super::seed_interview::{self, SeedInterviewInput, SeedInterviewPayload};
use async_graphql::{Context, Object};

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

    async fn add_asset(
        &self,
        ctx: &Context<'_>,
        input: AddAssetInput,
    ) -> async_graphql::Result<AddAssetPayload> {
        add_asset::add_asset(ctx, input).await
    }

    async fn edit_statement(
        &self,
        ctx: &Context<'_>,
        input: EditStatementInput,
    ) -> async_graphql::Result<EditStatementPayload> {
        edit_statement::edit_statement(ctx, input).await
    }
}
