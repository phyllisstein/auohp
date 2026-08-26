use super::captions::{self, Captions};
use super::interviews;
use super::search::{self, SearchHit};
use crate::graphql::nodes::{Interview, Statement, StatementNode, Transcript};
use crate::neo4j::Db;
use anyhow::Result;
use async_graphql::{Context, Error as GqlErr, Object};
use neo4rs::{BoltType, query};
use std::collections::HashMap;

#[derive(Default)]
pub struct QueryRoot;

#[Object]
impl QueryRoot {
    /// Returns "ok". Useful for readiness and liveness probes.
    async fn health(&self, _ctx: &Context<'_>) -> &'static str {
        "ok"
    }

    /// Lists all interviews in the archive, ordered by date.
    async fn interviews(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<Interview>> {
        todo!()
    }

    /// Fetches one interview by number
    async fn interview(
        &self,
        ctx: &Context<'_>,
        interview_number: i64,
    ) -> async_graphql::Result<Interview> {
        let db = ctx.data::<Db>()?;
        let mut params = HashMap::<&str, BoltType>::new();
        let mut to_return = Vec::<&str>::new();

        let mut query_statements =
            vec!["MATCH (interview:Interview {number: $rs.interviewNumber})"];
        params.insert("interviewNumber", BoltType::from(interview_number));
        to_return.push("interview");

        let look = ctx.look_ahead();
        let transcript_field = look.field("transcript");
        if transcript_field.exists() {
            query_statements
                .push("OPTIONAL MATCH (interview)-[:HAS_TRANSCRIPT]->(transcript:Transcript)");
            to_return.push("transcript");
        }

        let ret = format!("RETURN {returnables}", returnables = to_return.join(", "));
        query_statements.push(&ret);

        let query_str = query_statements.join("\n");
        let param_map = BoltType::from(params);

        let _ = db.execute(query(&query_str).param("rs", param_map)).await?;

        Err(GqlErr {
            message: "Not implemented".into(),
            source: None,
            extensions: None,
        })
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
    ) -> async_graphql::Result<Captions> {
        Err(GqlErr {
            message: "Not implemented".into(),
            source: None,
            extensions: None,
        })
    }

    async fn search_interview(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "Search keyword")] query: String,
        #[graphql(desc = "Interview UID")] uid: String,
    ) -> async_graphql::Result<Vec<SearchHit>> {
        search::search_interview(ctx, query, uid).await
    }

    async fn span_at_time(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "Find span at this time")] timestamp: f64,
        #[graphql(desc = "Find spans from interview")] interview_number: i64,
    ) -> async_graphql::Result<Statement> {
        Err(GqlErr {
            message: "Not implemented".into(),
            source: None,
            extensions: None,
        })
    }
}
