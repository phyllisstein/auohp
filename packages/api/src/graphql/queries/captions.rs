use crate::graphql::nodes::{Statement, StatementNode};
use crate::graphql::row::RowExt;
use crate::neo4j::Db;
use async_graphql::{Context, Object};
use neo4rs::query;

pub struct Captions {
    interview_number: i64,
}

#[Object]
impl Captions {
    async fn span_at_time(
        &self,
        ctx: &Context<'_>,
        timestamp: f64,
    ) -> async_graphql::Result<Option<Statement>> {
        let db = ctx.data::<Db>()?;
        let interview_number = self.interview_number;

        tracing::info!(timestamp, interview_number, "span at time");

        let mut span_stream = db
            .execute(
                query(
                    "
                    MATCH
                        (:Interview {number: $interviewNumber})-[:HAS_TRANSCRIPT]->
                        ()-[meta:CONTAINS]->
                        (span:Statement)
                    WHERE meta.startTime <= $timestamp AND meta.endTime >= $timestamp
                    RETURN span, meta
                    ORDER BY meta.startTime DESC
                    LIMIT 1
            ",
                )
                .param("interviewNumber", interview_number)
                .param("timestamp", timestamp),
            )
            .await?;

        // No statement spans this timestamp. That is a legitimate answer, not a
        // failure, so it becomes a null rather than an entry in the `errors`
        // array --- which is the whole reason this resolver returns
        // `Result<Option<Statement>>` rather than `Result<Statement>`. An error
        // here would propagate up to the nearest nullable ancestor, and with a
        // non-null field it would erase all of `data`.
        let Some(row) = span_stream.next().await? else {
            return Ok(None);
        };

        let sn = row.node_as::<StatementNode>("span")?;
        let start_time = row.rel_prop("meta", "startTime")?;
        let end_time = row.rel_prop("meta", "endTime")?;

        Ok(Some({
            Statement {
                uid: sn.uid,
                text: sn.text,
                person: None,
                start_time,
                end_time,
                words: None,
            }
        }))
    }
}

#[derive(Default)]
pub struct CaptionsQuery;

#[Object]
impl CaptionsQuery {
    async fn captions(&self, interview_number: i64) -> Captions {
        Captions { interview_number }
    }
}
