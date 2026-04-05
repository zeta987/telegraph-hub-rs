use reqwest::Client;

use crate::error::AppError;
use crate::telegraph::types::*;

const BASE_URL: &str = "https://api.telegra.ph";

/// Parameters for creating or editing a Telegraph page.
pub struct PageParams<'a> {
    pub access_token: &'a str,
    pub title: &'a str,
    pub content: &'a str,
    pub author_name: Option<&'a str>,
    pub author_url: Option<&'a str>,
    pub return_content: bool,
}

/// Telegraph API client.
///
/// Wraps a `reqwest::Client` and provides typed methods
/// for every Telegraph API endpoint.
#[derive(Debug, Clone)]
pub struct TelegraphClient {
    http: Client,
}

impl TelegraphClient {
    pub fn new(http: Client) -> Self {
        Self { http }
    }

    // ── Account endpoints ──────────────────────────────────────────

    /// Create a new Telegraph account.
    pub async fn create_account(
        &self,
        short_name: &str,
        author_name: Option<&str>,
        author_url: Option<&str>,
    ) -> Result<Account, AppError> {
        let mut params = vec![("short_name", short_name.to_string())];
        if let Some(v) = author_name {
            params.push(("author_name", v.to_string()));
        }
        if let Some(v) = author_url {
            params.push(("author_url", v.to_string()));
        }

        let resp: ApiResponse<Account> = self
            .http
            .post(format!("{BASE_URL}/createAccount"))
            .form(&params)
            .send()
            .await?
            .json()
            .await?;

        unwrap_response(resp)
    }

    /// Edit an existing Telegraph account's info.
    pub async fn edit_account_info(
        &self,
        access_token: &str,
        short_name: Option<&str>,
        author_name: Option<&str>,
        author_url: Option<&str>,
    ) -> Result<Account, AppError> {
        let mut params = vec![("access_token", access_token.to_string())];
        if let Some(v) = short_name {
            params.push(("short_name", v.to_string()));
        }
        if let Some(v) = author_name {
            params.push(("author_name", v.to_string()));
        }
        if let Some(v) = author_url {
            params.push(("author_url", v.to_string()));
        }

        let resp: ApiResponse<Account> = self
            .http
            .post(format!("{BASE_URL}/editAccountInfo"))
            .form(&params)
            .send()
            .await?
            .json()
            .await?;

        unwrap_response(resp)
    }

    /// Get account information.
    pub async fn get_account_info(
        &self,
        access_token: &str,
        fields: Option<&[&str]>,
    ) -> Result<Account, AppError> {
        let fields_json = fields.map(|f| serde_json::to_string(f).unwrap_or_default());
        let mut params = vec![("access_token", access_token.to_string())];
        if let Some(f) = &fields_json {
            params.push(("fields", f.clone()));
        }

        let resp: ApiResponse<Account> = self
            .http
            .post(format!("{BASE_URL}/getAccountInfo"))
            .form(&params)
            .send()
            .await?
            .json()
            .await?;

        unwrap_response(resp)
    }

    /// Revoke the current access token and get a new one.
    pub async fn revoke_access_token(&self, access_token: &str) -> Result<Account, AppError> {
        let params = [("access_token", access_token)];

        let resp: ApiResponse<Account> = self
            .http
            .post(format!("{BASE_URL}/revokeAccessToken"))
            .form(&params)
            .send()
            .await?
            .json()
            .await?;

        unwrap_response(resp)
    }

    // ── Page endpoints ─────────────────────────────────────────────

    /// Create a new Telegraph page.
    pub async fn create_page(&self, p: &PageParams<'_>) -> Result<Page, AppError> {
        let mut params = vec![
            ("access_token", p.access_token.to_string()),
            ("title", p.title.to_string()),
            ("content", p.content.to_string()),
            ("return_content", p.return_content.to_string()),
        ];
        if let Some(v) = p.author_name {
            params.push(("author_name", v.to_string()));
        }
        if let Some(v) = p.author_url {
            params.push(("author_url", v.to_string()));
        }

        let resp: ApiResponse<Page> = self
            .http
            .post(format!("{BASE_URL}/createPage"))
            .form(&params)
            .send()
            .await?
            .json()
            .await?;

        unwrap_response(resp)
    }

    /// Edit an existing Telegraph page.
    pub async fn edit_page(&self, path: &str, p: &PageParams<'_>) -> Result<Page, AppError> {
        let mut params = vec![
            ("access_token", p.access_token.to_string()),
            ("title", p.title.to_string()),
            ("content", p.content.to_string()),
            ("return_content", p.return_content.to_string()),
        ];
        if let Some(v) = p.author_name {
            params.push(("author_name", v.to_string()));
        }
        if let Some(v) = p.author_url {
            params.push(("author_url", v.to_string()));
        }

        let resp: ApiResponse<Page> = self
            .http
            .post(format!("{BASE_URL}/editPage/{path}"))
            .form(&params)
            .send()
            .await?
            .json()
            .await?;

        unwrap_response(resp)
    }

    /// Get a Telegraph page by path.
    pub async fn get_page(&self, path: &str, return_content: bool) -> Result<Page, AppError> {
        let resp: ApiResponse<Page> = self
            .http
            .get(format!("{BASE_URL}/getPage/{path}"))
            .query(&[("return_content", return_content.to_string())])
            .send()
            .await?
            .json()
            .await?;

        unwrap_response(resp)
    }

    /// List pages belonging to an account.
    pub async fn get_page_list(
        &self,
        access_token: &str,
        offset: Option<i32>,
        limit: Option<i32>,
    ) -> Result<PageList, AppError> {
        let mut params = vec![("access_token", access_token.to_string())];
        if let Some(o) = offset {
            params.push(("offset", o.to_string()));
        }
        if let Some(l) = limit {
            params.push(("limit", l.to_string()));
        }

        let resp: ApiResponse<PageList> = self
            .http
            .get(format!("{BASE_URL}/getPageList"))
            .query(&params)
            .send()
            .await?
            .json()
            .await?;

        unwrap_response(resp)
    }

    /// Get view count for a page.
    pub async fn get_views(
        &self,
        path: &str,
        year: Option<i32>,
        month: Option<i32>,
        day: Option<i32>,
        hour: Option<i32>,
    ) -> Result<PageViews, AppError> {
        let mut params: Vec<(&str, String)> = Vec::new();
        if let Some(v) = year {
            params.push(("year", v.to_string()));
        }
        if let Some(v) = month {
            params.push(("month", v.to_string()));
        }
        if let Some(v) = day {
            params.push(("day", v.to_string()));
        }
        if let Some(v) = hour {
            params.push(("hour", v.to_string()));
        }

        let resp: ApiResponse<PageViews> = self
            .http
            .get(format!("{BASE_URL}/getViews/{path}"))
            .query(&params)
            .send()
            .await?
            .json()
            .await?;

        unwrap_response(resp)
    }
}

/// Extract the result from a Telegraph API response, or return an error.
fn unwrap_response<T>(resp: ApiResponse<T>) -> Result<T, AppError> {
    if resp.ok {
        resp.result
            .ok_or_else(|| AppError::Telegraph("Missing result in successful response".into()))
    } else {
        Err(AppError::Telegraph(
            resp.error
                .unwrap_or_else(|| "Unknown Telegraph error".into()),
        ))
    }
}
