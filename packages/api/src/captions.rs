//! Caption generation --- transport-agnostic//!
//! Produces WebVTT text for an interview's statements. Both the GraphQL
//! `captions` resolver and the REST `/interview/{n}/captions` endpoint call
//! `generate_vtt`; neither transport's types appear here. The function takes a
//! `&Db` and returns a plain `String`, so it depends on the graph and nothing
//! about how the result is served. Dependency points inward: transports depend
//! on this module, never the reverse.

use crate::graphql::nodes::StatementNode;
use crate::neo4j::Db;
use anyhow::Result;
use chrono::TimeDelta;
use neo4rs::query;

// FIXME: reaching up into `graphql::nodes` for `StatementNode` is the wrong
// direction --- a transport-agnostic module should not depend on the GraphQL
// layer. This is the coupling `nodes.rs` already flags: it exists only because
// the node projections are still mis-homed under `graphql/`. Extracting captions
// is what surfaces the pressure to lift those projections into `auohp-core`,
// where both this module and the GraphQL layer could depend on them cleanly.

fn format_millisecond_timestamp(t: i64) -> String {
    let mut ts = TimeDelta::milliseconds(t);

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
pub async fn generate_vtt(db: &Db, interview_number: i64) -> Result<String> {
    let mut statement_stream = db
        .execute(query!(
            "
            MATCH
                (int:Interview {{number: {number}}})-[:HAS_TRANSCRIPT]->
                (:Transcript)-[meta:CONTAINS]->
                (statement:Statement)
            RETURN statement, meta.startTime as startTime, meta.endTime as endTime
            ORDER BY meta.startTime ASCENDING
        ",
            number = interview_number,
        ))
        .await?;

    let mut vtts: Vec<String> = vec!["WEBVTT\n".into()];

    while let Some(row) = statement_stream.next().await? {
        let statement: neo4rs::Node = row.get("statement")?;
        let sn: StatementNode = statement.to()?;

        let start_time: neo4rs::BoltFloat = row.get("startTime")?;
        let end_time: neo4rs::BoltFloat = row.get("endTime")?;

        let start_timestamp = format_millisecond_timestamp((start_time.value * 1_800.0) as i64);
        let end_timestamp = format_millisecond_timestamp((end_time.value * 1_000.0) as i64);

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
    use rand::Rng;

    #[test]
    fn creates_timestamp_from_fractional_seconds() {
        const SECONDS: i64 = 1_7947;
        const STAMP: &str = "00:00:17.947";

        let as_str = format_millisecond_timestamp(SECONDS);
        println!("ms: {SECONDS}");
        println!("timestamp: {as_str}");

        assert_eq!(as_str, STAMP);
    }

    #[test]
    fn pads_contiguous_timestamps() {
        let base_r: i64 = rand::thread_rng().gen_range(500..=5_000);

        let t1 = base_r + 500;
        let t2 = base_r - 500;
        let ts_1 = format_millisecond_timestamp(t1);
        let ts_2 = format_millisecond_timestamp(t2);

        println!("t1:\t\t{t1}");
        println!("t2:\t\t{t2}");
        println!("ts_1:\t{ts_1}");
        println!("ts_2:\t{ts_2}");

        assert_ne!(ts_1, ts_2);
    }
}
