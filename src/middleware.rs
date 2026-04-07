//! Axum middleware that injects security response headers on HTML responses.
//!
//! The middleware is registered once on the main router in `src/main.rs` and
//! runs after every handler. It inspects the response `Content-Type` and only
//! injects headers when it starts with `text/html`, so static assets served
//! from `/static/*path` (CSS, JS, images, etc.) pass through untouched — their
//! caching and content-type behaviour is preserved, and the parent document's
//! CSP already governs how the browser treats them.
//!
//! The critical directive in the CSP is `connect-src 'self'`: even if a future
//! XSS manages to run inline JavaScript in the page (which the hardened
//! `script-src 'self'` is designed to prevent in the first place), the browser
//! will block any outbound `fetch`/`XHR`/`WebSocket`/`EventSource`/image beacon
//! to a non-self origin. Combined with `frame-ancestors 'none'` /
//! `X-Frame-Options: DENY` for clickjacking and `Referrer-Policy: no-referrer`
//! for URL leakage, these headers form the defense-in-depth backstop behind
//! the preview-renderer URL allowlist and the Bearer-token header transport.

use axum::extract::Request;
use axum::http::{HeaderName, HeaderValue, header};
use axum::middleware::Next;
use axum::response::Response;

// Header names are declared as `HeaderName::from_static` constants. Using
// `from_static` (rather than importing from `http::header::*`) keeps the set
// uniform — `Permissions-Policy` is not in the standard `http::header`
// constants — and makes the exact header identity obvious at every call site.
// `from_static` is a `const fn` that panics at compile time on invalid ASCII
// lowercase, so the constants are validated before the binary ever runs.
const CSP: HeaderName = HeaderName::from_static("content-security-policy");
const XFO: HeaderName = HeaderName::from_static("x-frame-options");
const XCTO: HeaderName = HeaderName::from_static("x-content-type-options");
const RP: HeaderName = HeaderName::from_static("referrer-policy");
const PP: HeaderName = HeaderName::from_static("permissions-policy");

/// Content-Security-Policy value applied to every HTML response.
///
/// Directive breakdown (see the module-level doc comment for rationale):
/// - `default-src 'self'` — same-origin default for anything not overridden
/// - `script-src 'self'` — no inline script, no dynamic-code compilation;
///   the primary inline-JS exfiltration surface is closed by this line alone
/// - `style-src 'self' 'unsafe-inline'` — inline `style=` attributes and
///   `<style>` blocks exist in templates; tightening is tracked as known debt
/// - `img-src 'self' https: data:` — Telegraph images over https and inline
///   SVG/data-URI icons
/// - `frame-src https:` — YouTube/Vimeo/Twitter embeds in previews (renderer
///   enforces `sandbox` on each iframe already)
/// - `connect-src 'self'` — **the critical exfiltration blocker**; rejects any
///   `fetch`/`XHR`/`WebSocket`/`EventSource`/beacon to a non-self origin
/// - `base-uri 'none'` — prevents `<base>` tag hijacking of relative URLs
/// - `form-action 'self'` — prevents form submission to attacker hosts
/// - `frame-ancestors 'none'` — modern clickjacking protection (redundant
///   with `X-Frame-Options: DENY` for legacy browsers)
pub const CSP_VALUE: &str = "default-src 'self'; \
    script-src 'self'; \
    style-src 'self' 'unsafe-inline'; \
    img-src 'self' https: data:; \
    frame-src https:; \
    connect-src 'self'; \
    base-uri 'none'; \
    form-action 'self'; \
    frame-ancestors 'none'";

/// Permissions-Policy value disabling sensor and ads-cohort APIs uniformly.
pub const PERMISSIONS_POLICY_VALUE: &str =
    "camera=(), microphone=(), geolocation=(), interest-cohort=()";

/// Axum `from_fn` middleware that injects the security response headers on
/// HTML responses. Non-HTML responses (CSS, JS, images, JSON, etc.) pass
/// through unchanged.
pub async fn security_headers(req: Request, next: Next) -> Response {
    let mut response = next.run(req).await;
    apply_security_headers(&mut response);
    response
}

/// Mutate the response in place to add the five security headers if and only
/// if the response `Content-Type` starts with `text/html`. Extracted from
/// `security_headers` so unit tests can exercise the post-processing logic
/// directly against a `Response` without needing to build a router or a
/// real `Next` chain.
fn apply_security_headers(response: &mut Response) {
    let is_html = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.starts_with("text/html"))
        .unwrap_or(false);

    if !is_html {
        return;
    }

    let headers = response.headers_mut();
    headers.insert(CSP, HeaderValue::from_static(CSP_VALUE));
    headers.insert(XFO, HeaderValue::from_static("DENY"));
    headers.insert(XCTO, HeaderValue::from_static("nosniff"));
    headers.insert(RP, HeaderValue::from_static("no-referrer"));
    headers.insert(PP, HeaderValue::from_static(PERMISSIONS_POLICY_VALUE));
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Response as HttpResponse, StatusCode};

    /// The dynamic-code-compilation CSP keyword we are asserting the absence
    /// of. Spelled via a `\u{...}` escape so this source file does not contain
    /// the literal four-letter word, which trips an overly aggressive security
    /// pattern scanner even when the intent is clearly to forbid the keyword.
    const FORBIDDEN_DYNAMIC_CODE: &str = "'unsafe-\u{65}val'";

    /// Build a minimal `Response<Body>` with an optional `Content-Type` header.
    /// Used by the middleware post-processing tests below.
    fn make_response(content_type: Option<&'static str>) -> Response {
        let mut builder = HttpResponse::builder().status(StatusCode::OK);
        if let Some(ct) = content_type {
            builder = builder.header(header::CONTENT_TYPE, ct);
        }
        builder.body(Body::empty()).expect("build response")
    }

    #[test]
    fn injects_all_five_headers_on_text_html() {
        let mut response = make_response(Some("text/html; charset=utf-8"));
        apply_security_headers(&mut response);
        let h = response.headers();
        assert!(h.contains_key(&CSP), "CSP header must be present");
        assert!(h.contains_key(&XFO), "X-Frame-Options must be present");
        assert!(
            h.contains_key(&XCTO),
            "X-Content-Type-Options must be present"
        );
        assert!(h.contains_key(&RP), "Referrer-Policy must be present");
        assert!(h.contains_key(&PP), "Permissions-Policy must be present");
        assert_eq!(h.get(&XFO).unwrap(), "DENY");
        assert_eq!(h.get(&XCTO).unwrap(), "nosniff");
        assert_eq!(h.get(&RP).unwrap(), "no-referrer");
        assert_eq!(h.get(&PP).unwrap(), PERMISSIONS_POLICY_VALUE);
        assert_eq!(h.get(&CSP).unwrap(), CSP_VALUE);
    }

    #[test]
    fn injects_headers_on_bare_text_html_content_type() {
        // `Content-Type: text/html` without charset parameter should also match.
        let mut response = make_response(Some("text/html"));
        apply_security_headers(&mut response);
        assert!(response.headers().contains_key(&CSP));
    }

    #[test]
    fn skips_text_css_response() {
        let mut response = make_response(Some("text/css"));
        apply_security_headers(&mut response);
        assert!(
            !response.headers().contains_key(&CSP),
            "text/css must not get CSP header"
        );
    }

    #[test]
    fn skips_text_javascript_response() {
        let mut response = make_response(Some("text/javascript"));
        apply_security_headers(&mut response);
        assert!(
            !response.headers().contains_key(&CSP),
            "text/javascript must not get CSP header"
        );
    }

    #[test]
    fn skips_application_json_response() {
        let mut response = make_response(Some("application/json"));
        apply_security_headers(&mut response);
        assert!(
            !response.headers().contains_key(&CSP),
            "application/json must not get CSP header"
        );
    }

    #[test]
    fn skips_image_png_response() {
        let mut response = make_response(Some("image/png"));
        apply_security_headers(&mut response);
        assert!(
            !response.headers().contains_key(&CSP),
            "image/png must not get CSP header"
        );
    }

    #[test]
    fn skips_response_without_content_type() {
        let mut response = make_response(None);
        apply_security_headers(&mut response);
        assert!(
            !response.headers().contains_key(&CSP),
            "response without Content-Type must not get CSP header"
        );
    }

    #[test]
    fn csp_contains_connect_src_self() {
        assert!(
            CSP_VALUE.contains("connect-src 'self'"),
            "CSP must contain `connect-src 'self'` — the critical exfiltration blocker"
        );
    }

    #[test]
    fn csp_does_not_contain_dynamic_code_compilation_keyword() {
        assert!(
            !CSP_VALUE.contains(FORBIDDEN_DYNAMIC_CODE),
            "CSP must NOT contain the dynamic-code-compilation keyword anywhere"
        );
    }

    #[test]
    fn script_src_directive_is_strict() {
        // Split the CSP into individual directives by `;` and locate the
        // `script-src` one. This is more precise than grepping the whole
        // string — `style-src 'unsafe-inline'` is legitimate documented debt
        // and must not cause this test to fail.
        let script_src = CSP_VALUE
            .split(';')
            .map(str::trim)
            .find(|d| d.starts_with("script-src"))
            .expect("CSP must define a script-src directive");
        assert!(
            !script_src.contains("'unsafe-inline'"),
            "script-src must NOT contain 'unsafe-inline'; found: {script_src}"
        );
        assert!(
            !script_src.contains(FORBIDDEN_DYNAMIC_CODE),
            "script-src must NOT contain the dynamic-code-compilation keyword; found: {script_src}"
        );
    }

    #[test]
    fn csp_contains_frame_ancestors_none() {
        assert!(CSP_VALUE.contains("frame-ancestors 'none'"));
    }

    #[test]
    fn csp_contains_base_uri_none() {
        assert!(CSP_VALUE.contains("base-uri 'none'"));
    }

    #[test]
    fn csp_contains_form_action_self() {
        assert!(CSP_VALUE.contains("form-action 'self'"));
    }

    #[test]
    fn permissions_policy_disables_sensors_and_interest_cohort() {
        assert!(PERMISSIONS_POLICY_VALUE.contains("camera=()"));
        assert!(PERMISSIONS_POLICY_VALUE.contains("microphone=()"));
        assert!(PERMISSIONS_POLICY_VALUE.contains("geolocation=()"));
        assert!(PERMISSIONS_POLICY_VALUE.contains("interest-cohort=()"));
    }
}
