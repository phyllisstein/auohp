//! Application-wide error type for REST handlers.
//!
//! In the GraphQL era, `gql_err` converted any `Display`-able error into an
//! `async_graphql::Error` string. Now that responses are plain JSON, we need
//! the error type to implement `axum::response::IntoResponse` so axum can
//! automatically turn a `Result<T, AppError>` return value into an HTTP
//! response with the right status code and JSON body.
//!
//! The key trait here is `IntoResponse`. Axum's handler infrastructure calls
//! `into_response()` on the return type of every handler function. By
//! implementing it for `AppError`, we get to decide the status code and body
//! shape for every failure path in the application.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;

/// A typed application error that carries an HTTP status code and a message.
///
/// `thiserror::Error` generates a `std::error::Error` impl from the `#[error]`
/// attribute on each variant. That gives us `Display` and `Error` for free,
/// which makes `?` propagation work in handler functions.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// The client referred to a resource that does not exist.
    #[error("not found: {0}")]
    NotFound(String),

    /// Something went wrong on the server (Neo4j, embedding, etc.).
    #[error("internal error: {0}")]
    Internal(String),

    /// The client sent a malformed request body.
    #[error("bad request: {0}")]
    BadRequest(String),
}

/// Convert `AppError` into an axum `Response`.
///
/// axum calls this automatically when a handler returns
/// `Result<impl IntoResponse, AppError>` and the result is `Err(_)`.
///
/// We serialize errors as `{"error": "...message..."}` JSON with the
/// appropriate status code so REST clients get structured error bodies rather
/// than bare text strings.
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match &self {
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
            AppError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        let body = json!({ "error": self.to_string() });
        (status, axum::Json(body)).into_response()
    }
}

/// Convert any `Display`-able error into an `AppError::Internal`.
///
/// This is the REST-era replacement for `gql_err`. Usage:
///
/// ```ignore
/// db.execute(query("...")).await.map_err(internal)?;
/// ```
pub fn internal(e: impl std::fmt::Display) -> AppError {
    AppError::Internal(e.to_string())
}
