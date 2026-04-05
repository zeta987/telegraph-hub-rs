use axum::Form;
use axum::extract::{Path, State};
use axum::response::Html;
use minijinja::context;
use serde::Deserialize;

use crate::AppState;
use crate::error::AppError;
use crate::telegraph::client::PageParams;

#[derive(Deserialize)]
pub struct ListPagesForm {
    pub access_token: String,
    pub offset: Option<i32>,
    pub limit: Option<i32>,
}

/// POST /pages/list — List pages for a given token.
pub async fn list_pages(
    State(state): State<AppState>,
    Form(form): Form<ListPagesForm>,
) -> Result<Html<String>, AppError> {
    let page_list = state
        .telegraph
        .get_page_list(
            &form.access_token,
            form.offset,
            Some(form.limit.unwrap_or(200)),
        )
        .await?;

    let tmpl = state.templates.get_template("page_list.html")?;
    let rendered = tmpl.render(context! {
        pages => page_list.pages,
        total_count => page_list.total_count,
        offset => form.offset.unwrap_or(0),
        limit => form.limit.unwrap_or(200),
    })?;
    Ok(Html(rendered))
}

/// GET /pages/edit/:path — Load the page editor with existing content.
pub async fn get_page_editor(
    State(state): State<AppState>,
    Path(path): Path<String>,
) -> Result<Html<String>, AppError> {
    let page = state.telegraph.get_page(&path, true).await?;
    let content_json = page
        .content
        .as_ref()
        .map(|c| serde_json::to_string_pretty(c).unwrap_or_default())
        .unwrap_or_default();

    let tmpl = state.templates.get_template("page_editor.html")?;
    let rendered = tmpl.render(context! {
        page,
        content_json,
        is_new => false,
    })?;
    Ok(Html(rendered))
}

/// GET /pages/new — Render an empty page editor for creating a new page.
pub async fn new_page_editor(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    let tmpl = state.templates.get_template("page_editor.html")?;
    let rendered = tmpl.render(context! {
        is_new => true,
        content_json => "[{\"tag\":\"p\",\"children\":[\"\"]}]",
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

    let tmpl = state.templates.get_template("fragments/toast.html")?;
    let rendered = tmpl.render(context! {
        message => format!("Page \"{}\" saved successfully!", page.title),
        variant => "success",
        url => page.url,
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

    let tmpl = state.templates.get_template("fragments/toast.html")?;
    let rendered = tmpl.render(context! {
        message => format!("Page \"{}\" created successfully!", page.title),
        variant => "success",
        url => page.url,
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

    let tmpl = state.templates.get_template("fragments/page_row.html")?;
    let rendered = tmpl.render(context! {
        path,
        title => "[DELETED]",
        deleted => true,
    })?;
    Ok(Html(rendered))
}
