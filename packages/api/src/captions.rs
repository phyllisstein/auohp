mod captions;

use axum::{
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use chrono::TimeDelta;

pub async fn get_captions(db: &Db, interview_number: i64) -> Result<Caption> {
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

        vtts.push(format!("{start_timestamp} --> {end_timestamp}\n{text}"))
    }

    let vtt = vtts.join("\n");

    Ok(Caption { vtt })
};


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
