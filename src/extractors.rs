use axum::extract::FromRequestParts;
use axum::http::{header, request::Parts};

use crate::error::AppError;

/// Error message returned for every rejection case from the `AccessToken`
/// extractor. Kept as a single constant so operators see a uniform toast.
const ERR_MSG: &str = "missing access token";

/// Axum extractor that reads `Authorization: Bearer <token>` from the request
/// headers and returns the bare token value.
///
/// Uses `FromRequestParts` so handlers can combine it with `Form<T>` extractors
/// for non-credential body fields on the same request. Reverse-proxy log
/// redaction tools (nginx, Caddy, Datadog, New Relic, etc.) mask the
/// `Authorization` header by default but log form bodies verbatim — routing
/// the credential through this header gives defense-in-depth against
/// accidental exposure in access logs.
#[derive(Debug)]
pub struct AccessToken(pub String);

impl<S: Send + Sync> FromRequestParts<S> for AccessToken {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let header_value = parts
            .headers
            .get(header::AUTHORIZATION)
            .ok_or_else(|| AppError::Telegraph(ERR_MSG.to_string()))?;

        // `to_str` rejects non-UTF-8 byte sequences without panicking, which
        // keeps garbage headers from tripping the extractor into an internal
        // server error.
        let header_str = header_value
            .to_str()
            .map_err(|_| AppError::Telegraph(ERR_MSG.to_string()))?;

        // The header must begin with exactly `Bearer ` (case-insensitive, a
        // single trailing space). `.get(..7)` is panic-free on short or
        // mid-UTF-8 inputs — it returns `None` instead of crashing.
        let Some(prefix) = header_str.get(..7) else {
            return Err(AppError::Telegraph(ERR_MSG.to_string()));
        };
        if !prefix.eq_ignore_ascii_case("Bearer ") {
            return Err(AppError::Telegraph(ERR_MSG.to_string()));
        }

        let token = &header_str[7..];
        // Strictness: real Telegraph tokens are opaque URL-safe base64 strings
        // with no whitespace. Reject empty tokens (e.g. `Bearer `) and any
        // token containing whitespace characters — this covers extra spaces
        // (`Bearer   abc`), tabs, trailing newlines, and mid-token spaces.
        if token.is_empty() || token.chars().any(|c| c.is_whitespace()) {
            return Err(AppError::Telegraph(ERR_MSG.to_string()));
        }

        Ok(AccessToken(token.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderValue, Request};

    /// Build a minimal `Request<()>` with an optional `Authorization` header
    /// and run the extractor against it.
    async fn extract(header_value: Option<HeaderValue>) -> Result<AccessToken, AppError> {
        let mut builder = Request::builder().uri("/");
        if let Some(h) = header_value {
            builder = builder.header("Authorization", h);
        }
        let req = builder.body(()).expect("build request");
        let (mut parts, _) = req.into_parts();
        AccessToken::from_request_parts(&mut parts, &()).await
    }

    /// Shorthand for `HeaderValue::from_str` on known-valid ASCII inputs.
    fn hv(s: &str) -> HeaderValue {
        HeaderValue::from_str(s).expect("build header value")
    }

    /// Assert an extractor error matches the missing-token contract.
    fn assert_missing_token(err: AppError) {
        match err {
            AppError::Telegraph(msg) => assert!(
                msg.contains("missing access token"),
                "expected missing-token message, got {msg:?}",
            ),
            other => panic!("expected Telegraph error, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn accepts_valid_bearer_header() {
        let token = extract(Some(hv("Bearer abc123"))).await.expect("extract");
        assert_eq!(token.0, "abc123");
    }

    #[tokio::test]
    async fn accepts_case_insensitive_prefix() {
        let t1 = extract(Some(hv("bearer abc123"))).await.expect("extract");
        assert_eq!(t1.0, "abc123");

        let t2 = extract(Some(hv("BEARER abc123"))).await.expect("extract");
        assert_eq!(t2.0, "abc123");

        let t3 = extract(Some(hv("BeArEr abc123"))).await.expect("extract");
        assert_eq!(t3.0, "abc123");
    }

    #[tokio::test]
    async fn rejects_missing_header() {
        let err = extract(None).await.expect_err("missing header must reject");
        assert_missing_token(err);
    }

    #[tokio::test]
    async fn rejects_basic_scheme() {
        let err = extract(Some(hv("Basic dXNlcjpwYXNz")))
            .await
            .expect_err("Basic scheme must reject");
        assert_missing_token(err);
    }

    #[tokio::test]
    async fn rejects_empty_token() {
        let err = extract(Some(hv("Bearer ")))
            .await
            .expect_err("empty token must reject");
        assert_missing_token(err);
    }

    #[tokio::test]
    async fn rejects_extra_spaces() {
        let err = extract(Some(hv("Bearer   abc123")))
            .await
            .expect_err("extra spaces must reject");
        assert_missing_token(err);
    }

    #[tokio::test]
    async fn rejects_non_utf8_bytes() {
        // Valid header value bytes but not valid UTF-8 after the `Bearer `
        // prefix. `HeaderValue::from_bytes` accepts 0x80-0xFF as obs-text per
        // RFC 7230, while `to_str` inside the extractor rejects them.
        let header_value =
            HeaderValue::from_bytes(&[b'B', b'e', b'a', b'r', b'e', b'r', b' ', 0x80, 0xFF])
                .expect("non-utf8 header value");
        let err = extract(Some(header_value))
            .await
            .expect_err("non-utf8 must reject");
        assert_missing_token(err);
    }
}
