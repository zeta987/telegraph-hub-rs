# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Development Commands

```bash
cargo build                       # Dev build
cargo run                         # Run server (default port 7890, auto-fallback if occupied)
cargo test                        # Run unit tests (types roundtrips, i18n locale consistency, Accept-Language parsing)
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
- **rusqlite** (bundled SQLite) for page cache persistence
- **dashmap** for lock-free concurrent in-memory cache

### AppState & Request Flow

`AppState` holds four shared resources: `TelegraphClient` (wraps `reqwest::Client`), `Arc<Environment>` (MiniJinja templates), `PageCache` (in-memory + optional SQLite persistence), and `Arc<I18n>` (translation maps for 3 locales). Handlers extract state via Axum's `State<AppState>`, call the Telegraph client, render a MiniJinja template, and return `Html<String>`. HTMX swaps the returned HTML fragment into the DOM.

### Key Patterns

**Node enum** (`src/telegraph/types.rs`): Telegraph content uses `#[serde(untagged)]` to represent nodes as either `Text(String)` or `Element(NodeElement { tag, attrs, children })`. This is the most critical serde design — changes here affect all page content handling.

**PageParams struct** (`src/telegraph/client.rs`): Groups page create/edit parameters using borrowed strings (`&'a str`) to satisfy clippy's `too_many_arguments` lint and avoid allocations.

**Error handling** (`src/error.rs`): `AppError` implements `IntoResponse` to return HTML toast fragments. Telegraph errors → 400, network errors → 502, template errors → 500. All error messages are HTML-escaped.

**Port fallback** (`src/main.rs`): Tries preferred port, then up to 9 consecutive ports if occupied. Logs a warning on fallback.

**Token storage & transport**: Access tokens live in browser `localStorage` (keyed by origin). Export/import as JSON file to migrate across ports. Server is fully stateless. Every request to `/pages/*` and `/account/*` (except `/account/create` and `/`) carries the token via the `Authorization: Bearer <token>` HTTP header — the server reads it through the `AccessToken` extractor in `src/extractors.rs` (`FromRequestParts`-based, case-insensitive `Bearer ` prefix, strict rejection of empty or whitespace-bearing tokens). Request bodies and query strings MUST NOT contain the token; reverse-proxy log redaction tools mask `Authorization` by default but log form bodies verbatim, so header transport gives defense-in-depth against accidental credential exposure in access logs. The HTMX client attaches the header via a `htmx:configRequest` hook in `static/app.js`; raw `fetch()` calls for the batch-delete and select-all paths set the header manually.

**Page cache** (`src/cache.rs` + `src/db.rs`): `PageCache` wraps `DashMap` for concurrent in-memory access with optional `Database` (SQLite, WAL mode) for persistence across restarts. A background `tokio::spawn` task fetches all pages for a token via batched `getPageList` calls (200 per batch, 50ms delay), storing `PageSummary` structs. Search filters this cached data. Progress is tracked via `AtomicUsize`/`AtomicBool` so the UI can show a progressive loading indicator while the cache builds. Entries expire after 5 minutes (configurable via `CACHE_TTL_SECS`). `BuildProgress` also carries a monotonic `AtomicU64` generation counter: `PageCache::invalidate` bumps it (with `Ordering::AcqRel`) before removing the `DashMap` entry, and background build workers capture the generation at start, re-check it inside both the final `inner.entry` atomic check-and-insert closure AND the SQLite `spawn_blocking` closure under the DB mutex. Mismatched generation aborts the build's write, closing the race where a late-arriving build would otherwise repopulate an invalidated cache entry and re-open the read window for an already-revoked access token.

**i18n** (`src/i18n.rs`): `I18n` struct holds `HashMap<locale, HashMap<key, value>>` loaded at startup from embedded JSON files (`locales/en.json`, `locales/zh-TW.json`, `locales/zh-CN.json`). A MiniJinja global function `t(key, **kwargs)` reads the `lang` variable from template context and interpolates `{var}` placeholders. The `Lang` extractor (`FromRequestParts`) resolves locale from: 1) `lang` cookie, 2) `Accept-Language` header (with quality sorting and zh-Hant/zh-Hans normalization), 3) default `"en"`. Keys prefixed with `js.*` are exposed to the client via a `<script type="application/json" id="i18n-data">` block in `base.html`; `static/app.js` parses it on `DOMContentLoaded` and assigns to `window.i18n`. The non-JavaScript MIME type keeps the data block outside the reach of the strict `script-src 'self'` CSP.

**Content rendering** (`src/telegraph/render.rs`): `render_nodes_to_html()` converts a Telegraph `Node` tree to sanitized HTML for inline preview. Only tags from the Telegraph API whitelist are rendered; unknown tags are stripped.

**Security response headers** (`src/middleware.rs`): A single `security_headers` function registered as `.layer(axum::middleware::from_fn(security_headers))` on the main router injects five headers on every response whose `Content-Type` starts with `text/html` — `Content-Security-Policy`, `X-Frame-Options: DENY`, `X-Content-Type-Options: nosniff`, `Referrer-Policy: no-referrer`, and `Permissions-Policy: camera=(), microphone=(), geolocation=(), interest-cohort=()`. Non-HTML responses (CSS, JS, images served from `/static/*path`) are left alone so their caching and MIME behaviour are preserved. The CSP value is `default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' https: data:; frame-src https:; connect-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'`. The critical directive is `connect-src 'self'`, which blocks every outbound `fetch` / `XHR` / `WebSocket` / `EventSource` / image beacon to a non-self origin — even if a hypothetical XSS slipped past the renderer allowlist, it could not exfiltrate tokens to an attacker host. `'unsafe-inline'` is allowed in `style-src` only (inline `style=` and `<style>` blocks still exist in templates) and is tracked as documented debt; `script-src` is strict with no inline, no nonce, and no dynamic-code compilation. **Template convention**: do NOT introduce inline `<script>` blocks, `on*=` event handlers, or HTMX `hx-vals='js:...'` expressions — they will be blocked by the CSP. Event handling uses delegation via `data-click` / `data-change` / `data-keydown` / `data-submit` attributes dispatched from `static/app.js`; server-known `hx-vals` payloads use pure JSON with the MiniJinja `| tojson | e` pipeline. To relax the CSP for an operator deployment that needs external image hosts or analytics, edit the `CSP_VALUE` constant in `src/middleware.rs` (e.g. change `img-src 'self' https: data:` to also include a specific `https://cdn.example.com` origin). HSTS is deliberately not set because the app runs on arbitrary ports and is self-hosted.

### Template Loading

All templates are loaded in `load_templates()` in `main.rs` via `include_str!()` and registered with MiniJinja using `add_template_owned()`. When adding a new template: create the `.html` file in `templates/`, then add the corresponding `add_template_owned()` call.

### Static Assets

Files in `static/` are embedded via `#[derive(Embed)] #[folder = "static/"]` and served at `/static/*path` with auto-detected MIME types. Rebuild required after adding/changing static files.

## Route Structure

All `/pages/*` and `/account/*` endpoints except `/account/create` and `/` require an `Authorization: Bearer <token>` header. The token never appears in the request body or query string.

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
| `/pages/search` | POST | `pages::search_pages` | Search pages (uses server-side cache) |
| `/pages/preview/{*path}` | GET | `pages::preview_page` | Inline content preview |
| `/pages/delete/{*path}` | POST | `pages::delete_page` | Soft-delete (overwrites with [DELETED]) |
| `/pages/batch-delete` | POST | `pages::batch_delete` | Batch soft-delete (JSON response) |
| `/pages/paths` | POST | `pages::get_page_paths` | All cached paths as JSON (for select-all) |
| `/lang/set` | POST | `lang::set_language` | Set UI language cookie + redirect |

## Configuration

| Environment Variable | Default | Description |
|---------------------|---------|-------------|
| `PORT` | `7890` | HTTP server port |
| `RUST_LOG` | `telegraph_hub_rs=info` | Log level filter |
| `LOG_DIR` | *(disabled)* | Directory for daily rolling log files; unset = stdout only |
| `LOG_TZ` | `local` | Log timestamp timezone; only when `LOG_DIR` is set. `local`, `UTC`, `+8`, `+09:00`, `UTC+8` |
| `TELEGRAPH_HUB_DB` | `telegraph_hub_cache.db` | SQLite cache database path (set to customize location; falls back to in-memory if open fails) |

## Telegraph API

Base URL: `https://api.telegra.ph`. All endpoints use `application/x-www-form-urlencoded`. The `TelegraphClient` in `src/telegraph/client.rs` wraps every endpoint. Responses follow `{ "ok": bool, "result": T, "error": string }` — parsed via `ApiResponse<T>` and unwrapped by `unwrap_response()`.

## Design Context

The UI follows **Terminal-flavored Dark Minimalism**: dark-native, performance-first, system-font-only, zero-shadow, zero-gradient, zero-filter. Target is WCAG AAA (7:1 contrast) with `prefers-reduced-motion` honored absolutely. The interface must run smoothly on aging hardware (10-year-old Android, Intel HD 4000 laptops) — this is a core requirement, not an edge case.

**Quick token reference:**
- Background: `#1a1c22` (dark default) / `#fafbfc` (light fallback)
- Primary text: `#f5f6f8` (15.76:1) / `#1a1c22`
- Accent: `#9ab5ff` — single cool blue-violet, AAA 8.45:1, used only for primary button / active page / focus ring / link
- Border: `rgba(255,255,255,0.10)` on dark, never solid dark lines
- Font: system stack only (`-apple-system` + `ui-monospace`), no `@font-face`
- Radius: `2px` buttons, `4px` containers, `9999px` chips (binary system, nothing in between)
- Transition: `120ms ease`, `opacity` + `background-color` only
- Elevation: background opacity stepping, never `box-shadow`
- Data table: htop-flavored, no zebra striping, row hover as only separation, tabular numerics, uppercase monospace column headers
- Default theme: `dark` (light is fallback for `prefers-color-scheme: light`)

**Hard prohibitions:**
- No `box-shadow`, `linear-gradient`, `backdrop-filter`, or `filter` effects
- No web font downloads (Inter, Geist, SF Pro, etc.)
- No `transform` animations (except the existing `spin` loading indicator)
- No `color-mix()` (old Safari compatibility)
- No radius values between `4px` and `9999px`
- No font weights `300`, `700`, `800`, `900`
- No text below 7:1 contrast if it needs to be read
- No zebra striping on tables

**Full specification:** See `.impeccable.md` at the project root for the complete design system, token tables with exact contrast ratios, do/don't lists, file impact scope, and the `impeccable:*` skill chain build order. All `impeccable:*` skills and UI-related work must read that file first before making any visual decisions.
