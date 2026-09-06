use axum::{http::StatusCode, response::{IntoResponse, Response}, Json};
use serde_json::json;

pub type Result<T> = std::result::Result<T, AppError>;

#[derive(Debug)]
pub struct AppError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
}
impl AppError {
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self { status, code, message: message.into() }
    }
    pub fn bad(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "VALIDATION_ERROR", message)
    }
    pub fn unauthorized() -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "UNAUTHORIZED", "Please sign in again")
    }
    pub fn missing() -> Self {
        Self::new(StatusCode::NOT_FOUND, "NOT_FOUND", "Resource not found")
    }
    pub fn internal() -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", "Operation failed; check the server's diagnostic code")
    }
}
impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.code)
    }
}
impl std::error::Error for AppError {}
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let id = uuid::Uuid::new_v4().to_string();
        let mut r = (self.status, Json(json!({"error": {"code": self.code, "message": self.message, "request_id": id}}))).into_response();
        if let Ok(v) = id.parse() { r.headers_mut().insert("x-request-id", v); }
        r
    }
}
impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        if matches!(e, sqlx::Error::RowNotFound) { return Self::missing(); }
        // Never log SQL bind values, database error detail or credentials.
        let code = e.as_database_error().and_then(|d| d.code()).map(|c| c.into_owned());
        tracing::error!(database_code = ?code, "database operation failed");
        Self::internal()
    }
}
impl From<serde_json::Error> for AppError {
    fn from(_: serde_json::Error) -> Self { Self::bad("Invalid JSON structure") }
}
