use crate::captions;
use crate::graphql::error::gql_err;
use crate::graphql::nodes::StatementNode;
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
) -> async_graphql::Result<StatementNode> {
    let db = ctx.data::<Db>()?;

    let mut span_stream = db
        .execute(query!(
            "
            (:Interview {{number: {interview_number}}}) -[meta:HAS_TRANSCRIPT]->(span:Statement)
                WHERE meta.startTime >= {timestamp} AND meta.endTime <= {timestamp}
            RETURN span
            LIMIT 1
        ",
            interview_number = interview_number,
            timestamp = timestamp,
        ))
        .await
        .map_err(gql_err)?;

    let s_row = span_stream.single().await.map_err(gql_err)?;
    let s: StatementNode = s_row.get("span").map_err(gql_err)?;

    Ok(s)
}
