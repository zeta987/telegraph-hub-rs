# Telegraph Hub RS

[正體中文](README.zh-TW.md)

A self-hosted web UI for managing [Telegraph](https://telegra.ph) pages, built with Rust.

No more manually crafting HTTP requests in Postman — manage your Telegraph accounts and pages from a clean web interface.

## Features

- **Account Management**: Create new Telegraph accounts, view account info, edit account details, revoke & regenerate tokens
- **Page Management**: List all pages, create new pages, edit existing pages, soft-delete pages
- **Token Manager**: Save and switch between multiple Telegraph accounts (stored in browser `localStorage`). Supports export/import as JSON file for backup or migrating across ports
- **Single Binary**: All assets embedded at compile time — deploy a single executable
- **Dark Mode**: Automatic dark/light theme based on system preference, with manual toggle
- **No JavaScript Framework**: HTMX-powered interactivity with zero build toolchain

## Tech Stack

| Component | Technology |
|-----------|-----------|
| Backend | [Axum](https://github.com/tokio-rs/axum) 0.8 |
| Templates | [MiniJinja](https://github.com/mitsuhiko/minijinja) 2 |
| Frontend | [HTMX](https://htmx.org/) 2 (vendored) |
| HTTP Client | [reqwest](https://github.com/seanmonstar/reqwest) (rustls) |
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

| Environment Variable | Default | Description |
|---------------------|---------|-------------|
| `PORT` | `7890` | HTTP server port |
| `RUST_LOG` | `telegraph_hub_rs=info` | Log level filter |

## Usage

1. **Open** `http://localhost:7890` in your browser
2. **Create** a new Telegraph account or **import** an existing access token
3. **Select** a token from the dropdown — pages load automatically
4. Use **Edit** / **Delete** buttons in the page list, or click **+ New Page** to create one

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
