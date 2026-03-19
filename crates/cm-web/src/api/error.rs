//! Shared API error type.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use cm_core::CmError;

/// API error that converts CmError variants to appropriate HTTP responses.
pub struct ApiError(pub CmError);

impl From<CmError> for ApiError {
    fn from(err: CmError) -> Self {
        Self(err)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match &self.0 {
            CmError::EntryNotFound(_) => (StatusCode::NOT_FOUND, self.0.to_string()),
            CmError::ScopeNotFound(_) => (StatusCode::NOT_FOUND, self.0.to_string()),
            CmError::DuplicateContent(_) => (StatusCode::CONFLICT, self.0.to_string()),
            CmError::InvalidScopePath(_) => (StatusCode::BAD_REQUEST, self.0.to_string()),
            CmError::InvalidEntryKind(_) => (StatusCode::BAD_REQUEST, self.0.to_string()),
            CmError::InvalidRelationKind(_) => (StatusCode::BAD_REQUEST, self.0.to_string()),
            CmError::Validation(_) => (StatusCode::BAD_REQUEST, self.0.to_string()),
            CmError::ConstraintViolation(_) => (StatusCode::CONFLICT, self.0.to_string()),
            CmError::Json(_) => (StatusCode::BAD_REQUEST, self.0.to_string()),
            CmError::Database(_) | CmError::Internal(_) => {
                tracing::error!(error = %self.0, "internal error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal server error".to_owned(),
                )
            }
        };

        if status.is_client_error() {
            tracing::debug!(status = status.as_u16(), error = %message, "client error");
        }

        (status, axum::Json(serde_json::json!({ "error": message }))).into_response()
    }
}
