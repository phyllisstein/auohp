use crate::graphql::error::gql_err;
use crate::graphql::interviews::{Person, Statement, StatementNode};
use crate::neo4j::Db;
use async_graphql::{Context, SimpleObject};
use chrono::TimeDelta;
use neo4rs::query;

#[derive(SimpleObject)]
pub struct Caption {
    vtt: String,
}

fn to_timestamp(&t: &TimeDelta) -> String {
    let mut ts = t;

    let hours = ts.num_hours();
    ts -= TimeDelta::hours(hours);
    let minutes = ts.num_minutes();
    ts -= TimeDelta::minutes(minutes);
    let seconds = ts.num_seconds();
    ts -= TimeDelta::seconds(seconds);
    let ms = ts.num_milliseconds();

    format!("{hours:02}:{minutes:02}:{seconds:02}.{ms:03}")
}

pub async fn get_captions(
    ctx: &Context<'_>,
    interview_number: i64,
) -> async_graphql::Result<Caption> {
    let db = ctx.data::<Db>()?;

    let mut statement_stream = db
        .execute(query!(
            "
            MATCH
                (int:Interview {{number: {interviewNumber}}})-[:HAS_TRANSCRIPT]->
                (:Transcript)-[meta:CONTAINS]->
                (statement:Statement)
            RETURN statement, meta.startTime as startTime, meta.endTime as endTime
            ORDER BY meta.startTime ASCENDING
            LIMIT 25
        ",
            interviewNumber = interview_number,
        ))
        .await
        .map_err(gql_err)?;

    let mut vtts: Vec<String> = vec!["WEBVTT\n".into()];

    while let Some(row) = statement_stream.next().await.map_err(gql_err)? {
        let statement: neo4rs::Node = row.get("statement").map_err(gql_err)?;
        let sn: StatementNode = statement.to().map_err(gql_err)?;

        let start_time: neo4rs::BoltFloat = row.get("startTime").map_err(gql_err)?;
        let end_time: neo4rs::BoltFloat = row.get("endTime").map_err(gql_err)?;

        let start = TimeDelta::milliseconds((start_time.value * 1_000.0) as i64);
        let end = TimeDelta::milliseconds((end_time.value * 1_000.0) as i64);

        let start_timestamp = to_timestamp(&start);
        let end_timestamp = to_timestamp(&end);
        let text = sn.text.clone();

        vtts.push(format!("{start_timestamp} --> {end_timestamp}\n{text}"))
    }

    let vtt = vtts.join("\n");

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_timestamp_from_fractional_seconds() {
        const SECONDS: f64 = 1794.7;

        let ts = TimeDelta::milliseconds((SECONDS * 1_000.0) as i64);
        let as_str = to_timestamp(&ts);
        assert_eq!(as_str, "00:29:54.700");
    }
}
