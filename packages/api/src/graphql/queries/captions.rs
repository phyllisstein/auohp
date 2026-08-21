use crate::captions;
use crate::graphql::error::gql_err;
use crate::graphql::nodes::{Statement, StatementNode};
use crate::neo4j::Db;
use async_graphql::{Context, SimpleObject};
use neo4rs::query;

#[derive(SimpleObject)]
pub struct Caption {
    vtt: String,
}

/// Thin GraphQL adapter over the transport-agnostic `captions::generate_vtt`:
/// pull the db out of the request context, delegate, and wrap the VTT string in
/// the GraphQL `Caption` object.
pub async fn get_captions(
    ctx: &Context<'_>,
    interview_uid: &str,
) -> async_graphql::Result<Caption> {
    let db = ctx.data::<Db>()?;
    let vtt = captions::generate_vtt(db, &interview_uid)
        .await
        .map_err(gql_err)?;
    Ok(Caption { vtt })
}

pub async fn span_at_time(
    ctx: &Context<'_>,
    timestamp: f64,
    interview_number: i64,
) -> async_graphql::Result<Statement> {
    let db = ctx.data::<Db>()?;

    tracing::info!(timestamp, interview_number, "span at time");

    let mut span_stream = db
        .execute(
            query(
                "
                    MATCH
                        (:Interview {number: $interviewNumber})-[:HAS_TRANSCRIPT]->
                        ()-[meta:CONTAINS]->
                        (span:Statement)
                    WHERE meta.startTime >= $timestamp
                    RETURN span, meta
                    ORDER BY meta.startTime
                    LIMIT 1
            ",
            )
            .param("interviewNumber", interview_number)
            .param("timestamp", timestamp),
        )
        .await
        .map_err(gql_err)?;

    let s_row = span_stream.single().await.map_err(gql_err)?;
    let statement: neo4rs::Node = s_row.get("span").map_err(gql_err)?;
    let meta: neo4rs::Relation = s_row.get("meta").map_err(gql_err)?;
    let sn: StatementNode = statement.to().map_err(gql_err)?;

    Ok(Statement {
        uid: sn.uid,
        text: sn.text,
        person: None,
        start_time: meta.get("startTime").map_err(gql_err)?,
        end_time: meta.get("endTime").map_err(gql_err)?,
        words: None,
    })
}
