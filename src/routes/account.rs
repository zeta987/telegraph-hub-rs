use axum::Form;
use axum::extract::State;
use axum::response::Html;
use minijinja::context;
use serde::Deserialize;

use crate::AppState;
use crate::error::AppError;

/// GET / — Render the landing page / token manager.
pub async fn index(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    let tmpl = state.templates.get_template("index.html")?;
    let rendered = tmpl.render(context! {})?;
    Ok(Html(rendered))
}

#[derive(Deserialize)]
pub struct CreateAccountForm {
    pub short_name: String,
    pub author_name: Option<String>,
    pub author_url: Option<String>,
}

/// POST /account/create — Create a new Telegraph account.
pub async fn create_account(
    State(state): State<AppState>,
    Form(form): Form<CreateAccountForm>,
) -> Result<Html<String>, AppError> {
    let account = state
        .telegraph
        .create_account(
            &form.short_name,
            form.author_name.as_deref(),
            form.author_url.as_deref(),
        )
        .await?;

    let tmpl = state
        .templates
        .get_template("fragments/account_card.html")?;
    let rendered = tmpl.render(context! { account, is_new => true })?;
    Ok(Html(rendered))
}

#[derive(Deserialize)]
pub struct TokenForm {
    pub access_token: String,
}

/// POST /account/info — Get account info for a given token.
pub async fn get_account_info(
    State(state): State<AppState>,
    Form(form): Form<TokenForm>,
) -> Result<Html<String>, AppError> {
    let fields = &["short_name", "author_name", "author_url", "page_count"];
    let account = state
        .telegraph
        .get_account_info(&form.access_token, Some(fields))
        .await?;

    let tmpl = state
        .templates
        .get_template("fragments/account_card.html")?;
    let rendered = tmpl.render(context! { account, is_new => false })?;
    Ok(Html(rendered))
}

#[derive(Deserialize)]
pub struct EditAccountForm {
    pub access_token: String,
    pub short_name: Option<String>,
    pub author_name: Option<String>,
    pub author_url: Option<String>,
}

/// POST /account/edit — Edit account info.
pub async fn edit_account_info(
    State(state): State<AppState>,
    Form(form): Form<EditAccountForm>,
) -> Result<Html<String>, AppError> {
    let account = state
        .telegraph
        .edit_account_info(
            &form.access_token,
            form.short_name.as_deref(),
            form.author_name.as_deref(),
            form.author_url.as_deref(),
        )
        .await?;

    let tmpl = state
        .templates
        .get_template("fragments/account_card.html")?;
    let rendered = tmpl.render(context! { account, is_new => false })?;
    Ok(Html(rendered))
}

/// POST /account/revoke — Revoke access token and get a new one.
pub async fn revoke_access_token(
    State(state): State<AppState>,
    Form(form): Form<TokenForm>,
) -> Result<Html<String>, AppError> {
    let account = state
        .telegraph
        .revoke_access_token(&form.access_token)
        .await?;

    let tmpl = state
        .templates
        .get_template("fragments/account_card.html")?;
    let rendered = tmpl.render(context! { account, is_new => true })?;
    Ok(Html(rendered))
}
