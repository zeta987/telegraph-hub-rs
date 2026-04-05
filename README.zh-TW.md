# Telegraph Hub RS

[English](README.md)

一個以 Rust 打造的自架式 Telegraph (telegra.ph) 頁面管理網頁介面。

不用再開啟 Postman 手動拼 HTTP 請求了——直接在瀏覽器裡管理你的 Telegraph 帳號和頁面。

## 功能特色

- **帳號管理**：建立新的 Telegraph 帳號、檢視帳號資訊、編輯帳號設定、復原並重新產生 access token
- **頁面管理**：列出所有頁面、建立新頁面、編輯既有頁面、軟刪除頁面
- **Token 管理器**：在瀏覽器中儲存並切換多組 Telegraph 帳號（資料存於 `localStorage`），支援 JSON 檔匯出/匯入，方便備份或在不同 port 之間遷移
- **單一執行檔**：所有靜態資源在編譯時期嵌入——部署時只需一個執行檔
- **深色模式**：自動依系統偏好切換深色/淺色主題，也可手動切換
- **零前端建置工具**：使用 HTMX 驅動互動，不需要 npm 或任何 JavaScript 框架

## 技術堆疊

| 元件 | 技術 |
|------|------|
| 後端框架 | [Axum](https://github.com/tokio-rs/axum) 0.8 |
| 範本引擎 | [MiniJinja](https://github.com/mitsuhiko/minijinja) 2 |
| 前端互動 | [HTMX](https://htmx.org/) 2（已內嵌） |
| HTTP 用戶端 | [reqwest](https://github.com/seanmonstar/reqwest)（rustls） |
| 靜態資源嵌入 | [rust-embed](https://github.com/pyrossh/rust-embed) |

## 快速開始

### 前置需求

- [Rust](https://rustup.rs/)（1.85 以上，支援 edition 2024）

### 建置與執行

```bash
# 複製專案
git clone https://github.com/zeta987/telegraph-hub-rs.git
cd telegraph-hub-rs

# 建置並執行
cargo run

# 或建置 release 版本
cargo build --release
./target/release/telegraph-hub-rs
```

伺服器預設啟動於 `http://localhost:7890`。若該 port 被佔用，會自動嘗試往上遞增（最多嘗試 10 個連續 port）。

### 環境變數設定

| 環境變數 | 預設值 | 說明 |
|---------|--------|------|
| `PORT` | `7890` | HTTP 伺服器 port |
| `RUST_LOG` | `telegraph_hub_rs=info` | 日誌等級篩選器 |

## 使用方式

1. 在瀏覽器開啟 `http://localhost:7890`
2. 建立新的 Telegraph 帳號，或匯入既有的 access token
3. 從頂部下拉選單選擇要使用的 token——頁面列表會自動載入
4. 在頁面列表中使用 **Edit** / **Delete** 按鈕操作，或點選 **+ New Page** 建立新頁面

### Token 儲存機制

Access token 儲存在瀏覽器的 `localStorage` 中，依 origin（協定 + 主機 + port）隔離。伺服器本身完全無狀態，不會儲存任何 token。

由於 `localStorage` 按 port 隔離，變更伺服器 port 後先前儲存的 token 不會自動帶過去。請利用 Saved Tokens 區塊的 **Export** / **Import File** 按鈕，將 token 匯出為 `telegraph-hub-tokens.json` 檔案，再於新 port 匯入即可。

### Telegraph API 支援範圍

| 端點 | 狀態 |
|------|------|
| `createAccount` | 已支援 |
| `editAccountInfo` | 已支援 |
| `getAccountInfo` | 已支援 |
| `revokeAccessToken` | 已支援 |
| `createPage` | 已支援 |
| `editPage` | 已支援 |
| `getPage` | 已支援 |
| `getPageList` | 已支援 |
| `getViews` | 已支援 |

## 開發指引

```bash
# 搭配 cargo-watch 進行熱重載開發
cargo watch -x run

# 靜態分析
cargo clippy -- -D warnings

# 程式碼格式化
cargo fmt

# 執行測試
cargo test
```

## 授權條款

[MIT](LICENSE)
