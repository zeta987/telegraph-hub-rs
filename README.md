# Telegraph Hub RS

[正體中文](README.zh-TW.md) | [简体中文](README.zh-CN.md)

A self-hosted web UI for managing [Telegraph](https://telegra.ph) pages, built with Rust.

No more manually crafting HTTP requests in Postman — manage your Telegraph accounts and pages from a clean web interface.

## Features

- **Account Management**: Create new Telegraph accounts, view account info, edit account details, revoke & regenerate tokens
- **Page Management**: List all pages, create new pages, edit existing pages, soft-delete pages
- **Token Manager**: Save and switch between multiple Telegraph accounts (stored in browser `localStorage`). Supports export/import as JSON file for backup or migrating across ports
- **Single Binary**: All assets embedded at compile time — deploy a single executable
- **Search & Pagination**: Full-text search across all pages with progressive loading; paginated page list with configurable page size
- **Batch Operations**: Select multiple pages and delete them in one operation (rate-limited to respect Telegraph API limits)
- **Inline Preview**: Preview page content directly in the UI without opening telegra.ph
- **Multilingual UI**: English, Traditional Chinese (繁體中文), Simplified Chinese (简体中文); auto-detects browser language, manual switch via navbar
- **Dark Mode**: Automatic dark/light theme based on system preference, with manual toggle
- **No JavaScript Framework**: HTMX-powered interactivity with zero build toolchain

## Tech Stack

| Component | Technology |
|-----------|-----------|
| Backend | [Axum](https://github.com/tokio-rs/axum) 0.8 |
| Templates | [MiniJinja](https://github.com/mitsuhiko/minijinja) 2 |
| Frontend | [HTMX](https://htmx.org/) 2 (vendored) |
| HTTP Client | [reqwest](https://github.com/seanmonstar/reqwest) (rustls) |
| Cache / DB | [rusqlite](https://github.com/rusqlite/rusqlite) (bundled SQLite) |
| Asset Embedding | [rust-embed](https://github.com/pyrossh/rust-embed) |

## Quick Start

### Prerequisites

- [Rust](https://rustup.rs/) (1.85+ with edition 2024 support)

### Build & Run

```bash
# Clone the repository
git clone https://github.com/zeta987/telegraph-hub-rs.git
cd telegraph-hub-rs

# Build and run
cargo run

# Or build a release binary
cargo build --release
./target/release/telegraph-hub-rs
```

The server starts at `http://localhost:7890` by default. If port 7890 is already in use, it automatically tries the next port (7891, 7892, ...) up to 10 attempts. Check the terminal output for the actual listening address.

### Configuration

Copy `.env.example` to `.env` and edit as needed:

```bash
cp .env.example .env
```

| Environment Variable | Default | Description |
|---------------------|---------|-------------|
| `PORT` | `7890` | HTTP server port |
| `RUST_LOG` | `telegraph_hub_rs=info` | Log level filter |
| `LOG_DIR` | *(disabled)* | Directory for daily rolling log files (e.g. `logs`) |
| `LOG_TZ` | `local` | Log timestamp timezone; only effective when `LOG_DIR` is set. Accepts `local`, `UTC`, `+8`, `+09:00`, `UTC+8`, `-5:30` |
| `TELEGRAPH_HUB_DB` | `telegraph_hub_cache.db` | SQLite cache database path |

#### Log Levels (`RUST_LOG`)

Uses [tracing-subscriber `EnvFilter`](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html) syntax. Levels from most to least verbose: `trace` > `debug` > `info` > `warn` > `error`.

```bash
# Default — info and above for this crate only
RUST_LOG=telegraph_hub_rs=info

# Development — see cache builds, API calls, retry details
RUST_LOG=telegraph_hub_rs=debug

# Quiet — only warnings and errors
RUST_LOG=telegraph_hub_rs=warn

# Multi-target — debug for this crate, warn for noisy dependencies
RUST_LOG=warn,telegraph_hub_rs=debug
```

## Usage

1. **Open** `http://localhost:7890` in your browser
2. **Create** a new Telegraph account or **import** an existing access token
3. **Select** a token from the dropdown — pages load automatically
4. Use **Edit** / **Delete** buttons in the page list, or click **+ New Page** to create one
5. **Search** across all pages using the search bar (builds a server-side cache on first use)
6. **Batch delete**: Toggle select mode, check multiple pages, then delete in one action
7. **Preview**: Click a page title to see an inline preview without leaving the app
8. **Language**: Switch between EN / 繁中 / 简中 via the navbar language buttons

### Token Storage

Access tokens are stored in your browser's `localStorage`, scoped to the origin (protocol + host + port). The server itself is completely stateless and never stores tokens.

Since `localStorage` is isolated per port, changing the server port means your saved tokens won't carry over. Use the **Export** / **Import File** buttons in the Saved Tokens section to migrate tokens between ports or back them up as a `telegraph-hub-tokens.json` file.

### Telegraph API Coverage

| Endpoint | Status |
|----------|--------|
| `createAccount` | Supported |
| `editAccountInfo` | Supported |
| `getAccountInfo` | Supported |
| `revokeAccessToken` | Supported |
| `createPage` | Supported |
| `editPage` | Supported |
| `getPage` | Supported |
| `getPageList` | Supported |
| `getViews` | Supported |

## Development

```bash
# Run with hot-reload (install cargo-watch first)
cargo watch -x run

# Lint
cargo clippy -- -D warnings

# Format
cargo fmt

# Test
cargo test
```

## License

[MIT](LICENSE)
