mod cache;
mod db;
mod error;
mod extractors;
pub mod i18n;
mod routes;
mod telegraph;

use std::sync::Arc;

use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Router, extract::Path as AxumPath};
use minijinja::Environment;
use rust_embed::Embed;
use tower_http::compression::CompressionLayer;

use crate::cache::PageCache;
use crate::db::Database;
use crate::i18n::I18n;
use crate::telegraph::client::TelegraphClient;

/// Embedded static assets (CSS, JS).
#[derive(Embed)]
#[folder = "static/"]
struct StaticAssets;

/// Shared application state, cloneable via Arc.
#[derive(Clone)]
pub struct AppState {
    pub telegraph: TelegraphClient,
    pub templates: Arc<Environment<'static>>,
    pub page_cache: PageCache,
    pub i18n: Arc<I18n>,
}

#[tokio::main]
async fn main() {
    // Load .env file (silently ignore if missing)
    dotenvy::dotenv().ok();

    // Initialize tracing (stdout + optional file logging via LOG_DIR)
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "telegraph_hub_rs=info".parse().unwrap());

    use tracing_subscriber::prelude::*;
    if let Ok(log_dir) = std::env::var("LOG_DIR") {
        // File logging enabled — resolve timezone from LOG_TZ (default: local)
        let offset = resolve_log_tz();
        let timer = tracing_subscriber::fmt::time::OffsetTime::new(
            offset,
            time::macros::format_description!(
                "[year]-[month]-[day] [hour]:[minute]:[second].[subsecond digits:3]"
            ),
        );
        let stdout_layer = tracing_subscriber::fmt::layer().with_timer(timer.clone());
        let file_appender = tracing_appender::rolling::daily(&log_dir, "telegraph-hub-rs.log");
        let file_layer = tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .with_timer(timer)
            .with_writer(file_appender);
        tracing_subscriber::registry()
            .with(env_filter)
            .with(stdout_layer)
            .with(file_layer)
            .init();
        tracing::debug!(
            "File logging enabled → {log_dir}/telegraph-hub-rs.log.<date> (UTC{offset})"
        );
    } else {
        // Stdout only — default timestamps
        let stdout_layer = tracing_subscriber::fmt::layer();
        tracing_subscriber::registry()
            .with(env_filter)
            .with(stdout_layer)
            .init();
    };

    tracing::debug!("Environment loaded (RUST_LOG active at debug level)");

    // Build HTTP client for Telegraph API
    let http_client = reqwest::Client::builder()
        .user_agent("telegraph-hub-rs/0.1.0")
        .build()
        .expect("failed to build HTTP client");

    // Open SQLite cache database
    let db_path =
        std::env::var("TELEGRAPH_HUB_DB").unwrap_or_else(|_| "telegraph_hub_cache.db".to_string());
    let db_path = std::path::Path::new(&db_path);
    tracing::info!("Cache database: {}", db_path.display());

    let page_cache = match Database::open(db_path) {
        Ok(db) => PageCache::new_with_db(db),
        Err(e) => {
            tracing::warn!("Failed to open cache database, running without persistence: {e}");
            PageCache::new()
        }
    };

    // Load i18n translations
    let i18n = Arc::new(I18n::load());

    // Load templates and register i18n translate function
    let mut env = Environment::new();
    env.set_auto_escape_callback(|_| minijinja::AutoEscape::Html);
    load_templates(&mut env);
    i18n::register_translate_function(&mut env, Arc::clone(&i18n));

    let state = AppState {
        telegraph: TelegraphClient::new(http_client),
        templates: Arc::new(env),
        page_cache,
        i18n,
    };

    // Build router
    let app = Router::new()
        // Pages
        .route("/", get(routes::account::index))
        // Language
        .route("/lang/set", post(routes::lang::set_language))
        // Account
        .route("/account/create", post(routes::account::create_account))
        .route("/account/info", post(routes::account::get_account_info))
        .route("/account/edit", post(routes::account::edit_account_info))
        .route(
            "/account/revoke",
            post(routes::account::revoke_access_token),
        )
        // Pages
        .route("/pages/list", post(routes::pages::list_pages))
        .route("/pages/search", post(routes::pages::search_pages))
        .route("/pages/new", get(routes::pages::new_page_editor))
        .route("/pages/new", post(routes::pages::create_page))
        .route("/pages/preview/{*path}", get(routes::pages::preview_page))
        .route("/pages/edit/{*path}", get(routes::pages::get_page_editor))
        .route("/pages/edit/{*path}", post(routes::pages::edit_page))
        .route("/pages/delete/{*path}", post(routes::pages::delete_page))
        .route("/pages/batch-delete", post(routes::pages::batch_delete))
        .route("/pages/paths", post(routes::pages::get_page_paths))
        // Static assets
        .route("/static/{*path}", get(serve_static))
        .layer(CompressionLayer::new())
        .with_state(state);

    // Bind and serve — try up to 10 consecutive ports if the preferred one is occupied
    let preferred_port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(7890);

    let mut listener = None;
    for port in preferred_port..preferred_port.saturating_add(10) {
        let addr = format!("0.0.0.0:{port}");
        match tokio::net::TcpListener::bind(&addr).await {
            Ok(l) => {
                if port != preferred_port {
                    tracing::warn!("Port {preferred_port} is in use, falling back to {port}");
                }
                tracing::info!("Telegraph Hub RS listening on http://localhost:{port}");
                listener = Some(l);
                break;
            }
            Err(e) => {
                tracing::debug!("Cannot bind to port {port}: {e}");
            }
        }
    }

    let listener = listener.unwrap_or_else(|| {
        panic!(
            "failed to bind to any port in range {}..{}",
            preferred_port,
            preferred_port + 10
        )
    });
    axum::serve(listener, app).await.expect("server error");
}

/// Serve embedded static assets with correct Content-Type.
async fn serve_static(AxumPath(path): AxumPath<String>) -> Response {
    match StaticAssets::get(&path) {
        Some(file) => {
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            ([(header::CONTENT_TYPE, mime.as_ref())], file.data.to_vec()).into_response()
        }
        None => (axum::http::StatusCode::NOT_FOUND, "Not found").into_response(),
    }
}

/// Load all HTML templates into the MiniJinja environment.
fn load_templates(env: &mut Environment<'static>) {
    // Base layout
    env.add_template_owned(
        "base.html".to_string(),
        include_str!("../templates/base.html").to_string(),
    )
    .expect("failed to load base.html");

    env.add_template_owned(
        "index.html".to_string(),
        include_str!("../templates/index.html").to_string(),
    )
    .expect("failed to load index.html");

    env.add_template_owned(
        "page_list.html".to_string(),
        include_str!("../templates/page_list.html").to_string(),
    )
    .expect("failed to load page_list.html");

    env.add_template_owned(
        "page_editor.html".to_string(),
        include_str!("../templates/page_editor.html").to_string(),
    )
    .expect("failed to load page_editor.html");

    env.add_template_owned(
        "account_info.html".to_string(),
        include_str!("../templates/account_info.html").to_string(),
    )
    .expect("failed to load account_info.html");

    // Fragments
    env.add_template_owned(
        "fragments/account_card.html".to_string(),
        include_str!("../templates/fragments/account_card.html").to_string(),
    )
    .expect("failed to load account_card.html");

    env.add_template_owned(
        "fragments/toast.html".to_string(),
        include_str!("../templates/fragments/toast.html").to_string(),
    )
    .expect("failed to load toast.html");

    env.add_template_owned(
        "fragments/page_row.html".to_string(),
        include_str!("../templates/fragments/page_row.html").to_string(),
    )
    .expect("failed to load page_row.html");

    env.add_template_owned(
        "fragments/search_progress.html".to_string(),
        include_str!("../templates/fragments/search_progress.html").to_string(),
    )
    .expect("failed to load search_progress.html");

    env.add_template_owned(
        "fragments/page_preview.html".to_string(),
        include_str!("../templates/fragments/page_preview.html").to_string(),
    )
    .expect("failed to load page_preview.html");
}

/// Resolve the UTC offset for log timestamps from the `LOG_TZ` env var.
///
/// Supported formats:
/// - `local` or unset → system local timezone
/// - `UTC` → UTC+0
/// - `+8`, `-5` → hour offset
/// - `+08:00`, `-05:30` → hour:minute offset
/// - `UTC+8`, `UTC-5:30` → same with UTC prefix
fn resolve_log_tz() -> time::UtcOffset {
    match std::env::var("LOG_TZ") {
        Ok(val) => parse_utc_offset(&val).unwrap_or_else(|| {
            eprintln!("Warning: invalid LOG_TZ value \"{val}\", falling back to local timezone");
            time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC)
        }),
        Err(_) => time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC),
    }
}

/// Parse a UTC offset string into a `time::UtcOffset`.
fn parse_utc_offset(s: &str) -> Option<time::UtcOffset> {
    let s = s.trim();
    if s.eq_ignore_ascii_case("local") {
        return time::UtcOffset::current_local_offset().ok();
    }
    if s.eq_ignore_ascii_case("UTC") || s == "+0" || s == "+00" || s == "+00:00" {
        return Some(time::UtcOffset::UTC);
    }

    // Strip optional "UTC" prefix: "UTC+8" → "+8"
    let s = s
        .strip_prefix("UTC")
        .or_else(|| s.strip_prefix("utc"))
        .unwrap_or(s);

    let (sign, rest) = if let Some(r) = s.strip_prefix('+') {
        (1i8, r)
    } else if let Some(r) = s.strip_prefix('-') {
        (-1i8, r)
    } else {
        return None;
    };

    let parts: Vec<&str> = rest.split(':').collect();
    let hours: i8 = parts.first()?.parse().ok()?;
    let minutes: i8 = parts.get(1).and_then(|m| m.parse().ok()).unwrap_or(0);
    time::UtcOffset::from_hms(sign * hours, sign * minutes, 0).ok()
}
