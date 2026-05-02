use std::fmt::Debug;

use crate::graphql::error::gql_err;
use crate::graphql::interviews::{Statement, StatementMeta, StatementNode};
use crate::neo4j::Db;
use async_graphql::{Context, SimpleObject};
use chrono::TimeDelta;
use neo4rs::query;

#[derive(SimpleObject)]
pub struct Caption {
    vtt: String,
}

fn to_timestamp(&t: &TimeDelta) -> String {
    let mut ts = t.clone();

    let hours = ts.num_hours();
    ts = ts - TimeDelta::hours(hours);
    let minutes = ts.num_minutes();
    ts = ts - TimeDelta::minutes(minutes);
    let seconds = ts.num_seconds();
    ts = ts - TimeDelta::seconds(seconds);
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
                (s:Statement)
            RETURN statement, meta AS statementMeta
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

        let statement_meta: neo4rs::Relation = row.get("statementMeta").map_err(gql_err)?;
        let mn: StatementMeta = statement_meta.to().map_err(gql_err)?;

        let start = TimeDelta::milliseconds((mn.start_time * 1_000.0) as i64);
        let end = TimeDelta::milliseconds((mn.end_time * 1_000.0) as i64);

        let start_timestamp = to_timestamp(&start);
        let end_timestamp = to_timestamp(&end);

        vtts.push(format!("{start_timestamp} --> {end_timestamp}\n{sn.text}"))
    }

    let vtt = vtts.join("\n".into());

    Ok(Caption { vtt })
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
