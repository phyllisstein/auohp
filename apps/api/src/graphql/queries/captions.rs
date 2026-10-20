use crate::graphql::nodes::{Statement, StatementNode};
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

        // No match is a null, not an error: an error on a non-null field would
        // propagate to the nearest nullable ancestor and erase all of `data`.
        let Some(row) = span_stream.next().await? else {
            return Ok(None);
        };

        let sn: StatementNode = row.get("span")?;
        let start_time = row.get::<neo4rs::Relation>("meta")?.get("startTime")?;
        let end_time = row.get::<neo4rs::Relation>("meta")?.get("endTime")?;

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
