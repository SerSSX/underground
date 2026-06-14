//! Single error type mapped to HTTP responses.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("database error")]
    Db(#[from] sqlx::Error),
    #[error("unauthorized")]
    Unauthorized,
    #[error("forbidden")]
    Forbidden,
    #[error("not found")]
    NotFound,
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Validation(String),
    #[error("internal error")]
    Internal,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match &self {
            AppError::Db(e) => {
                tracing::error!(error = %e, "database error");
                StatusCode::INTERNAL_SERVER_ERROR
            }
            AppError::Internal => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::Unauthorized => StatusCode::UNAUTHORIZED,
            AppError::Forbidden => StatusCode::FORBIDDEN,
            AppError::NotFound => StatusCode::NOT_FOUND,
            AppError::Conflict(_) => StatusCode::CONFLICT,
            AppError::Validation(_) => StatusCode::UNPROCESSABLE_ENTITY,
        };
        // Body carries only a safe message — never internal details (SEC).
        (status, Json(json!({ "error": self.to_string() }))).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status_of(e: AppError) -> StatusCode {
        e.into_response().status()
    }

    #[test]
    fn maps_variants_to_status_codes() {
        assert_eq!(
            status_of(AppError::Db(sqlx::Error::RowNotFound)),
            StatusCode::INTERNAL_SERVER_ERROR
        );
        assert_eq!(status_of(AppError::Internal), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(status_of(AppError::Unauthorized), StatusCode::UNAUTHORIZED);
        assert_eq!(status_of(AppError::Forbidden), StatusCode::FORBIDDEN);
        assert_eq!(status_of(AppError::NotFound), StatusCode::NOT_FOUND);
        assert_eq!(status_of(AppError::Conflict("x".into())), StatusCode::CONFLICT);
        assert_eq!(
            status_of(AppError::Validation("y".into())),
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }

    #[test]
    fn db_message_is_generic_without_leak() {
        assert_eq!(AppError::Db(sqlx::Error::RowNotFound).to_string(), "database error");
    }

    #[test]
    fn safe_messages_are_preserved() {
        assert_eq!(AppError::Conflict("email taken".into()).to_string(), "email taken");
        assert_eq!(AppError::Validation("bad email".into()).to_string(), "bad email");
    }
}
