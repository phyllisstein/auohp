//! Handler for VTT caption generation.
//!
//! Route:
//!
//!   GET /interviews/:number/captions  --- returns a WebVTT document

use axum::{
    extract::{Path, State},
    http::header,
    response::IntoResponse,
};
use chrono::TimeDelta;
use neo4rs::query;

use crate::AppState;
use crate::error::{AppError, internal};
use crate::models::StatementNode;

/// Format a `TimeDelta` as a WebVTT timestamp: `HH:MM:SS.mmm`.
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

/// Returns a WebVTT document for the given interview number.
///
/// The response `Content-Type` is `text/vtt` so browsers and media players
/// can consume it directly as a track source.
pub async fn get_captions(
    State(state): State<AppState>,
    Path(number): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    let mut statement_stream = state
        .db
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
            interviewNumber = number,
        ))
        .await
        .map_err(internal)?;

    let mut vtts: Vec<String> = vec!["WEBVTT\n".into()];

    while let Some(row) = statement_stream.next().await.map_err(internal)? {
        let statement: neo4rs::Node = row.get("statement").map_err(internal)?;
        let sn: StatementNode = statement.to().map_err(internal)?;

        let start_time: neo4rs::BoltFloat = row.get("startTime").map_err(internal)?;
        let end_time: neo4rs::BoltFloat = row.get("endTime").map_err(internal)?;

        let start = TimeDelta::milliseconds((start_time.value * 1_000.0) as i64);
        let end = TimeDelta::milliseconds((end_time.value * 1_000.0) as i64);

        let start_ts = to_timestamp(&start);
        let end_ts = to_timestamp(&end);

        vtts.push(format!("{start_ts} --> {end_ts}\n{}", sn.text));
    }

    let vtt = vtts.join("\n");

    // Return plain text with the VTT MIME type. axum's default JSON
    // serialization would double-encode this string; using a raw `Response`
    // with an explicit `Content-Type` header avoids that.
    Ok(([(header::CONTENT_TYPE, "text/vtt; charset=utf-8")], vtt))
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
