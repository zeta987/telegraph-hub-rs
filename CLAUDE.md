# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Development Commands

```bash
cargo build                       # Dev build
cargo run                         # Run server (default port 7890, auto-fallback if occupied)
cargo test                        # Run unit tests (types serialization roundtrips)
cargo clippy -- -D warnings       # Lint (must pass with zero warnings)
cargo fmt                         # Auto-format
cargo fmt --check                 # Check formatting without modifying
cargo build --release             # Release build (single binary, all assets embedded)
```

## Architecture Overview

**Single-binary Rust web app** that proxies the [Telegraph API](https://telegra.ph/api) through an HTMX-driven UI. The browser never talks to telegra.ph directly — all API calls go through the Axum backend.

### Core Stack
- **Axum 0.8** web framework with **MiniJinja 2** server-side templates
- **HTMX 2** (vendored in `static/`) for frontend interactivity — no JS framework, no npm
- **reqwest** (rustls-tls) as HTTP client for Telegraph API
- **rust-embed** + `include_str!()` embeds all templates and static assets at compile time

### AppState & Request Flow

`AppState` holds a `TelegraphClient` (wraps `reqwest::Client`) and `Arc<Environment>` (MiniJinja templates). Handlers extract state via Axum's `State<AppState>`, call the Telegraph client, render a MiniJinja template, and return `Html<String>`. HTMX swaps the returned HTML fragment into the DOM.

### Key Patterns

**Node enum** (`src/telegraph/types.rs`): Telegraph content uses `#[serde(untagged)]` to represent nodes as either `Text(String)` or `Element(NodeElement { tag, attrs, children })`. This is the most critical serde design — changes here affect all page content handling.

**PageParams struct** (`src/telegraph/client.rs`): Groups page create/edit parameters using borrowed strings (`&'a str`) to satisfy clippy's `too_many_arguments` lint and avoid allocations.

**Error handling** (`src/error.rs`): `AppError` implements `IntoResponse` to return HTML toast fragments. Telegraph errors → 400, network errors → 502, template errors → 500. All error messages are HTML-escaped.

**Port fallback** (`src/main.rs`): Tries preferred port, then up to 9 consecutive ports if occupied. Logs a warning on fallback.

**Token storage**: Access tokens live in browser `localStorage` (keyed by origin). Export/import as JSON file to migrate across ports. Server is fully stateless.

### Template Loading

All templates are loaded in `load_templates()` in `main.rs` via `include_str!()` and registered with MiniJinja using `add_template_owned()`. When adding a new template: create the `.html` file in `templates/`, then add the corresponding `add_template_owned()` call.

### Static Assets

Files in `static/` are embedded via `#[derive(Embed)] #[folder = "static/"]` and served at `/static/*path` with auto-detected MIME types. Rebuild required after adding/changing static files.

## Route Structure

| Route | Method | Handler | Purpose |
|-------|--------|---------|---------|
| `/` | GET | `account::index` | Home / token manager |
| `/account/create` | POST | `account::create_account` | Create Telegraph account |
| `/account/info` | POST | `account::get_account_info` | Fetch account details |
| `/account/edit` | POST | `account::edit_account_info` | Update account info |
| `/account/revoke` | POST | `account::revoke_access_token` | Revoke & regenerate token |
| `/pages/list` | POST | `pages::list_pages` | List pages for account |
| `/pages/new` | GET/POST | `pages::new_page_editor` / `create_page` | New page form / create |
| `/pages/edit/{*path}` | GET/POST | `pages::get_page_editor` / `edit_page` | Edit form / save changes |
| `/pages/delete/{*path}` | POST | `pages::delete_page` | Soft-delete (overwrites with [DELETED]) |

## Telegraph API

Base URL: `https://api.telegra.ph`. All endpoints use `application/x-www-form-urlencoded`. The `TelegraphClient` in `src/telegraph/client.rs` wraps every endpoint. Responses follow `{ "ok": bool, "result": T, "error": string }` — parsed via `ApiResponse<T>` and unwrapped by `unwrap_response()`.
