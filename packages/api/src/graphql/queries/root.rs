use async_graphql::{Context, Object};

use super::captions::{self, Caption};
use super::interviews;
use super::search::{self, SearchHit};
use crate::graphql::nodes::{Interview, Transcript};

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
    /// by start time (via the `startTime` property on the `:CONTAINS` relationship).
    async fn interview_transcript(
        &self,
        ctx: &Context<'_>,
        number: i64,
    ) -> async_graphql::Result<Transcript> {
        interviews::get_transcript(ctx, number).await
    }

    /// Semantic search over Statement text via the `statementEmbedding` vector
    /// index. Returns up to `limit` hits (default 15) ranked by cosine
    /// similarity, each carrying the matching statement and its parent
    /// interview.
    async fn search_statements(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "Natural-language query to embed and search for.")] query: String,
        #[graphql(desc = "Maximum number of results to return. Defaults to 15.")] limit: Option<
            i64,
        >,
    ) -> async_graphql::Result<Vec<SearchHit>> {
        search::search_statements(ctx, query, limit).await
    }

    async fn captions(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "Interview UID")] interview_number: String,
    ) -> async_graphql::Result<Caption> {
        captions::get_captions(ctx, &interview_number).await
    }
}
