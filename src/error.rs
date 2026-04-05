use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};

/// Application-wide error type.
#[derive(Debug)]
pub enum AppError {
    /// Error returned by the Telegraph API.
    Telegraph(String),
    /// HTTP request failure (network, timeout, etc.).
    Request(reqwest::Error),
    /// Template rendering failure.
    Template(minijinja::Error),
    /// SQLite database error.
    Database(rusqlite::Error),
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::Telegraph(msg) => write!(f, "Telegraph API error: {msg}"),
            AppError::Request(e) => write!(f, "HTTP request error: {e}"),
            AppError::Template(e) => write!(f, "Template error: {e}"),
            AppError::Database(e) => write!(f, "Database error: {e}"),
        }
    }
}

impl std::error::Error for AppError {}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        AppError::Request(e)
    }
}

impl From<minijinja::Error> for AppError {
    fn from(e: minijinja::Error) -> Self {
        AppError::Template(e)
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        AppError::Database(e)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::Telegraph(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::Request(e) => (
                StatusCode::BAD_GATEWAY,
                format!("Failed to reach Telegraph API: {e}"),
            ),
            AppError::Template(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Template rendering error: {e}"),
            ),
            AppError::Database(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Database error: {e}"),
            ),
        };

        tracing::error!("{self}");

        // Return an HTML error fragment suitable for HTMX swap or full-page display
        let html = format!(
            r#"<div class="toast toast-error" role="alert">
  <strong>Error</strong>
  <p>{}</p>
</div>"#,
            html_escape(&message)
        );

        (status, Html(html)).into_response()
    }
}

/// Minimal HTML escaping for error messages.
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
