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

        let s_row = span_stream.next().await?;

        let row = match s_row {
            Some(r) => r,
            None => {
                return Err(async_graphql::Error {
                    message: format!("No span returned at {timestamp}"),
                    source: None,
                    extensions: None,
                });
            }
        };

        let statement: neo4rs::Node = row.get("span")?;
        let meta: neo4rs::Relation = row.get("meta")?;
        let sn: StatementNode = statement.to()?;

        Ok(Some({
            Statement {
                uid: sn.uid,
                text: sn.text,
                person: None,
                start_time: meta.get("startTime")?,
                end_time: meta.get("endTime")?,
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
