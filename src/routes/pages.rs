use axum::Form;
use axum::Json;
use axum::extract::{Path, State};
use axum::response::Html;
use minijinja::context;
use serde::{Deserialize, Serialize};

use std::time::Duration;

use crate::AppState;
use crate::cache::hash_token;
use crate::error::AppError;
use crate::i18n::Lang;
use crate::telegraph::client::PageParams;
use crate::telegraph::render::render_nodes_to_html;

/// Compute a sliding window of up to 10 page numbers centered on `current_page`.
fn page_window(current_page: i64, total_pages: i64) -> (i64, i64) {
    const WINDOW: i64 = 10;
    let half = WINDOW / 2;
    let start = std::cmp::max(1, current_page - half);
    let end = std::cmp::min(total_pages, start + WINDOW - 1);
    let start = std::cmp::max(1, end - WINDOW + 1);
    (start, end)
}

#[derive(Deserialize)]
pub struct ListPagesForm {
    pub access_token: String,
    pub offset: Option<i32>,
    pub limit: Option<i32>,
}

/// POST /pages/list — List pages for a given token.
///
/// Uses the in-memory page cache when available for instant pagination.
/// Falls back to a direct Telegraph API call on cache miss, and triggers
/// a background cache build so subsequent page switches are fast.
pub async fn list_pages(
    State(state): State<AppState>,
    Lang(lang): Lang,
    Form(form): Form<ListPagesForm>,
) -> Result<Html<String>, AppError> {
    let limit = form.limit.unwrap_or(50);
    let offset = form.offset.unwrap_or(0);
    let token_hash = hash_token(&form.access_token);

    // Fast path: serve from cache if available
    if let Some(cached) = state.page_cache.get(&token_hash) {
        let total_count = cached.total_count;
        let total_pages = if total_count == 0 {
            1
        } else {
            ((total_count as f64) / (limit as f64)).ceil() as i64
        };
        let current_page = (offset as i64) / (limit as i64) + 1;
        let (page_start, page_end) = page_window(current_page, total_pages);

        let start = offset as usize;
        let end = std::cmp::min(start + limit as usize, cached.pages.len());
        let page_slice = if start < cached.pages.len() {
            &cached.pages[start..end]
        } else {
            &[]
        };

        let tmpl = state.templates.get_template("page_list.html")?;
        let rendered = tmpl.render(context! {
            pages => page_slice,
            total_count,
            offset,
            limit,
            current_page,
            total_pages,
            page_start,
            page_end,
            has_prev => offset > 0,
            has_next => (offset as i64 + limit as i64) < total_count,
            lang,
        })?;
        return Ok(Html(rendered));
    }

    // Slow path: no cache — call Telegraph API directly
    let page_list = state
        .telegraph
        .get_page_list(&form.access_token, Some(offset), Some(limit))
        .await?;

    // Trigger a background cache build so subsequent navigations are instant
    if !state.page_cache.is_building(&token_hash) {
        state.page_cache.start_build(
            token_hash,
            form.access_token.clone(),
            state.telegraph.clone(),
        );
    }

    let total_count = page_list.total_count;
    let total_pages = if total_count == 0 {
        1
    } else {
        ((total_count as f64) / (limit as f64)).ceil() as i64
    };
    let current_page = (offset as i64) / (limit as i64) + 1;

    let (page_start, page_end) = page_window(current_page, total_pages);

    let tmpl = state.templates.get_template("page_list.html")?;
    let rendered = tmpl.render(context! {
        pages => page_list.pages,
        total_count,
        offset,
        limit,
        current_page,
        total_pages,
        page_start,
        page_end,
        has_prev => offset > 0,
        has_next => (offset as i64 + limit as i64) < total_count,
        lang,
    })?;
    Ok(Html(rendered))
}

/// GET /pages/edit/:path — Load the page editor with existing content.
pub async fn get_page_editor(
    State(state): State<AppState>,
    Lang(lang): Lang,
    Path(path): Path<String>,
) -> Result<Html<String>, AppError> {
    let page = state.telegraph.get_page(&path, true).await?;
    let content_json = page
        .content
        .as_ref()
        .map(|c| serde_json::to_string_pretty(c).unwrap_or_default())
        .unwrap_or_default();

    let js_translations = state.i18n.js_translations(&lang);
    let tmpl = state.templates.get_template("page_editor.html")?;
    let rendered = tmpl.render(context! {
        page,
        content_json,
        is_new => false,
        lang,
        js_translations,
    })?;
    Ok(Html(rendered))
}

/// GET /pages/preview/:path — Render an inline preview of page content.
pub async fn preview_page(
    State(state): State<AppState>,
    Lang(lang): Lang,
    Path(path): Path<String>,
) -> Result<Html<String>, AppError> {
    let page = state.telegraph.get_page(&path, true).await?;
    let content_html = page
        .content
        .as_ref()
        .map(|nodes| render_nodes_to_html(nodes))
        .unwrap_or_default();

    let tmpl = state
        .templates
        .get_template("fragments/page_preview.html")?;
    let rendered = tmpl.render(context! {
        title => page.title,
        author_name => page.author_name,
        views => page.views,
        url => page.url,
        content => content_html,
        lang,
    })?;
    Ok(Html(rendered))
}

/// GET /pages/new — Render an empty page editor for creating a new page.
pub async fn new_page_editor(
    State(state): State<AppState>,
    Lang(lang): Lang,
) -> Result<Html<String>, AppError> {
    let js_translations = state.i18n.js_translations(&lang);
    let tmpl = state.templates.get_template("page_editor.html")?;
    let rendered = tmpl.render(context! {
        is_new => true,
        content_json => "[{\"tag\":\"p\",\"children\":[\"\"]}]",
        lang,
        js_translations,
    })?;
    Ok(Html(rendered))
}

#[derive(Deserialize)]
pub struct EditPageForm {
    pub access_token: String,
    pub title: String,
    pub content: String,
    pub author_name: Option<String>,
    pub author_url: Option<String>,
}

/// POST /pages/edit/:path — Save changes to an existing page.
pub async fn edit_page(
    State(state): State<AppState>,
    Lang(lang): Lang,
    Path(path): Path<String>,
    Form(form): Form<EditPageForm>,
) -> Result<Html<String>, AppError> {
    let params = PageParams {
        access_token: &form.access_token,
        title: &form.title,
        content: &form.content,
        author_name: form.author_name.as_deref(),
        author_url: form.author_url.as_deref(),
        return_content: false,
    };
    let page = state.telegraph.edit_page(&path, &params).await?;

    // Invalidate search cache for this token
    state.page_cache.invalidate(&hash_token(&form.access_token));

    let message = state
        .i18n
        .translate(&lang, "toast.page_saved", &[("title", &page.title)]);
    let tmpl = state.templates.get_template("fragments/toast.html")?;
    let rendered = tmpl.render(context! {
        message,
        variant => "success",
        url => page.url,
        lang,
    })?;
    Ok(Html(rendered))
}

#[derive(Deserialize)]
pub struct CreatePageForm {
    pub access_token: String,
    pub title: String,
    pub content: String,
    pub author_name: Option<String>,
    pub author_url: Option<String>,
}

/// POST /pages/new — Create a new Telegraph page.
pub async fn create_page(
    State(state): State<AppState>,
    Lang(lang): Lang,
    Form(form): Form<CreatePageForm>,
) -> Result<Html<String>, AppError> {
    let params = PageParams {
        access_token: &form.access_token,
        title: &form.title,
        content: &form.content,
        author_name: form.author_name.as_deref(),
        author_url: form.author_url.as_deref(),
        return_content: false,
    };
    let page = state.telegraph.create_page(&params).await?;

    // Invalidate search cache for this token
    state.page_cache.invalidate(&hash_token(&form.access_token));

    let message = state
        .i18n
        .translate(&lang, "toast.page_created", &[("title", &page.title)]);
    let tmpl = state.templates.get_template("fragments/toast.html")?;
    let rendered = tmpl.render(context! {
        message,
        variant => "success",
        url => page.url,
        lang,
    })?;
    Ok(Html(rendered))
}

#[derive(Deserialize)]
pub struct SearchPagesForm {
    pub access_token: String,
    pub query: String,
    pub offset: Option<i32>,
    pub limit: Option<i32>,
}

/// POST /pages/search — Search all pages (uses server-side cache).
///
/// Three states:
/// 1. Cache hit → return filtered results immediately
/// 2. Cache building (background task running) → return progress indicator
///    (the progress indicator auto-polls this same endpoint every second)
/// 3. No cache, no build → start background build, return progress indicator
pub async fn search_pages(
    State(state): State<AppState>,
    Lang(lang): Lang,
    Form(form): Form<SearchPagesForm>,
) -> Result<Html<String>, AppError> {
    let limit = form.limit.unwrap_or(50);
    let offset = form.offset.unwrap_or(0);
    let token_hash = hash_token(&form.access_token);

    // State 1: Cache hit → return results immediately
    if let Some(cached) = state.page_cache.get(&token_hash) {
        return render_search_results(
            &state,
            &lang,
            &cached.pages,
            &form.query,
            offset,
            limit,
            None,
        );
    }

    // State 2: Check if build is in progress → show partial results
    if let Some((fetched, total, complete, error)) = state.page_cache.get_progress(&token_hash) {
        if complete && let Some(err_msg) = error {
            let tmpl = state.templates.get_template("fragments/toast.html")?;
            let rendered = tmpl.render(context! {
                message => format!("Failed to build page cache: {err_msg}"),
                variant => "error",
                lang,
            })?;
            return Ok(Html(rendered));
        }
        // Build just completed — cache should now be available
        if complete && let Some(cached) = state.page_cache.get(&token_hash) {
            return render_search_results(
                &state,
                &lang,
                &cached.pages,
                &form.query,
                offset,
                limit,
                None,
            );
        }

        // Still building — search partial data and show results + progress banner
        if let Some(partial_pages) = state.page_cache.get_partial_pages(&token_hash) {
            return render_search_results(
                &state,
                &lang,
                &partial_pages,
                &form.query,
                offset,
                limit,
                Some((fetched, total)),
            );
        }
    }

    // State 3: No cache, no build → start background build
    state.page_cache.start_build(
        token_hash,
        form.access_token.clone(),
        state.telegraph.clone(),
    );

    // Return progress with zero results (will auto-poll in 1s)
    render_search_results(&state, &lang, &[], &form.query, offset, limit, Some((0, 0)))
}

/// Render search results: filter pages by query, paginate, render template.
/// When `is_building` is true, includes a progress banner that auto-polls.
fn render_search_results(
    state: &AppState,
    lang: &str,
    pages: &[crate::cache::PageSummary],
    query: &str,
    offset: i32,
    limit: i32,
    build: Option<(usize, usize)>,
) -> Result<Html<String>, AppError> {
    let (is_building, fetched, total) = match build {
        Some((f, t)) => (true, f, t),
        None => (false, 0, 0),
    };
    let query_lower = query.to_lowercase();
    let filtered: Vec<_> = pages
        .iter()
        .filter(|p| {
            p.title.to_lowercase().contains(&query_lower)
                || p.path.to_lowercase().contains(&query_lower)
        })
        .collect();

    let total_count = filtered.len() as i64;
    let total_pages = if total_count == 0 {
        1
    } else {
        ((total_count as f64) / (limit as f64)).ceil() as i64
    };
    let current_page = (offset as i64) / (limit as i64) + 1;

    let start = offset as usize;
    let end = std::cmp::min(start + limit as usize, filtered.len());
    let page_results: Vec<_> = if start < filtered.len() {
        filtered[start..end].to_vec()
    } else {
        vec![]
    };

    let (page_start, page_end) = page_window(current_page, total_pages);

    let tmpl = state.templates.get_template("page_list.html")?;
    let rendered = tmpl.render(context! {
        pages => page_results,
        total_count,
        offset,
        limit,
        current_page,
        total_pages,
        page_start,
        page_end,
        has_prev => offset > 0,
        has_next => (offset as i64 + limit as i64) < total_count,
        is_search => true,
        search_query => query,
        is_building,
        build_fetched => fetched,
        build_total => total,
        lang,
    })?;
    Ok(Html(rendered))
}

#[derive(Deserialize)]
pub struct DeletePageForm {
    pub access_token: String,
}

/// POST /pages/delete/:path — Soft-delete a page by overwriting with [DELETED].
pub async fn delete_page(
    State(state): State<AppState>,
    Lang(lang): Lang,
    Path(path): Path<String>,
    Form(form): Form<DeletePageForm>,
) -> Result<Html<String>, AppError> {
    let deleted_content = r#"[{"tag":"p","children":["Deleted"]}]"#;
    let params = PageParams {
        access_token: &form.access_token,
        title: "[DELETED]",
        content: deleted_content,
        author_name: None,
        author_url: None,
        return_content: false,
    };
    state.telegraph.edit_page(&path, &params).await?;

    // Invalidate search cache for this token
    state.page_cache.invalidate(&hash_token(&form.access_token));

    let tmpl = state.templates.get_template("fragments/page_row.html")?;
    let url = format!("https://telegra.ph/{path}");
    let rendered = tmpl.render(context! {
        path,
        title => "[DELETED]",
        deleted => true,
        url,
        lang,
    })?;
    Ok(Html(rendered))
}

#[derive(Deserialize)]
pub struct BatchDeleteForm {
    pub access_token: String,
    pub paths: String,
}

#[derive(Serialize)]
pub struct BatchDeleteResult {
    pub succeeded: Vec<String>,
    pub failed: Vec<BatchDeleteFailure>,
}

#[derive(Serialize)]
pub struct BatchDeleteFailure {
    pub path: String,
    pub error: String,
}

/// POST /pages/batch-delete — Batch soft-delete pages with rate limiting.
pub async fn batch_delete(
    State(state): State<AppState>,
    Form(form): Form<BatchDeleteForm>,
) -> Result<Json<BatchDeleteResult>, AppError> {
    let paths: Vec<&str> = form
        .paths
        .split(',')
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .collect();

    if paths.is_empty() {
        return Err(AppError::Telegraph("No pages specified.".into()));
    }
    if paths.len() > 50 {
        return Err(AppError::Telegraph(
            "Maximum batch size is 50 pages per request.".into(),
        ));
    }

    let deleted_content = r#"[{"tag":"p","children":["Deleted"]}]"#;
    let mut succeeded = Vec::new();
    let mut failed = Vec::new();

    for (i, path) in paths.iter().enumerate() {
        let params = PageParams {
            access_token: &form.access_token,
            title: "[DELETED]",
            content: deleted_content,
            author_name: None,
            author_url: None,
            return_content: false,
        };
        match state.telegraph.edit_page(path, &params).await {
            Ok(_) => succeeded.push(path.to_string()),
            Err(e) => failed.push(BatchDeleteFailure {
                path: path.to_string(),
                error: e.to_string(),
            }),
        }
        // Rate limit: 300ms between requests (skip after the last one)
        if i + 1 < paths.len() {
            tokio::time::sleep(Duration::from_millis(300)).await;
        }
    }

    // Update cache in-place (mark as deleted instead of full invalidation)
    state
        .page_cache
        .mark_deleted(&hash_token(&form.access_token), &succeeded);

    Ok(Json(BatchDeleteResult { succeeded, failed }))
}
