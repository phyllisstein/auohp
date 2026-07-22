//! Caption generation --- transport-agnostic.
//!
//! Produces WebVTT text for an interview's statements. Both the GraphQL
//! `captions` resolver and the REST `/interview/{n}/captions` endpoint call
//! `generate_vtt`; neither transport's types appear here. The function takes a
//! `&Db` and returns a plain `String`, so it depends on the graph and nothing
//! about how the result is served. Dependency points inward: transports depend
//! on this module, never the reverse.

use anyhow::Result;
use chrono::TimeDelta;
use neo4rs::query;

use crate::graphql::nodes::StatementNode;
use crate::neo4j::Db;

// FIXME: reaching up into `graphql::nodes` for `StatementNode` is the wrong
// direction --- a transport-agnostic module should not depend on the GraphQL
// layer. This is the coupling `nodes.rs` already flags: it exists only because
// the node projections are still mis-homed under `graphql/`. Extracting captions
// is what surfaces the pressure to lift those projections into `auohp-core`,
// where both this module and the GraphQL layer could depend on them cleanly.

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

/// Build a WebVTT document for an interview's statements, ordered by start time.
pub async fn generate_vtt(db: &Db, interview_uid: &str) -> Result<String> {
    let mut statement_stream = db
        .execute(query!(
            "
            MATCH
                (int:Interview {{uid: {uid}}})-[:HAS_TRANSCRIPT]->
                (:Transcript)-[meta:CONTAINS]->
                (statement:Statement)
            RETURN statement, meta.startTime as startTime, meta.endTime as endTime
            ORDER BY meta.startTime ASCENDING
        ",
            uid = interview_uid,
        ))
        .await?;

    let mut vtts: Vec<String> = vec!["WEBVTT\n".into()];

    while let Some(row) = statement_stream.next().await? {
        let statement: neo4rs::Node = row.get("statement")?;
        let sn: StatementNode = statement.to()?;

        let start_time: neo4rs::BoltFloat = row.get("startTime")?;
        let end_time: neo4rs::BoltFloat = row.get("endTime")?;

        let start = TimeDelta::milliseconds((start_time.value * 1_000.0) as i64);
        let end = TimeDelta::milliseconds((end_time.value * 1_000.0) as i64);

        let start_timestamp = to_timestamp(&start);
        let end_timestamp = to_timestamp(&end);
        let text = sn.text.clone();

        vtts.push(format!(
            "{suid}\n{start_timestamp} --> {end_timestamp}\n{text}\n",
            suid = sn.uid
        ))
    }

    Ok(vtts.join("\n"))
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
