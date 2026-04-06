use axum::Form;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

use crate::i18n::SUPPORTED_LOCALES;

#[derive(Deserialize)]
pub struct SetLangForm {
    pub lang: String,
    pub redirect: Option<String>,
}

/// POST /lang/set — Set the UI language cookie and redirect back.
pub async fn set_language(headers: HeaderMap, Form(form): Form<SetLangForm>) -> Response {
    let lang = if SUPPORTED_LOCALES.contains(&form.lang.as_str()) {
        &form.lang
    } else {
        "en"
    };

    // Determine redirect target: form field → Referer header → /
    let redirect_to = form.redirect.filter(|r| !r.is_empty()).unwrap_or_else(|| {
        headers
            .get(header::REFERER)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("/")
            .to_string()
    });

    let cookie = format!("lang={lang}; Path=/; SameSite=Lax; Max-Age=31536000");

    (
        StatusCode::SEE_OTHER,
        [
            (header::SET_COOKIE, cookie),
            (header::LOCATION, redirect_to),
        ],
    )
        .into_response()
}
