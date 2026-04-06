# Telegraph Hub RS

[正體中文](README.zh-TW.md) | [English](README.md)

一个以 Rust 打造的自托管 Telegraph (telegra.ph) 页面管理 Web 界面。

不用再打开 Postman 手动拼 HTTP 请求了——直接在浏览器里管理你的 Telegraph 账号和页面。

## 功能特色

- **账号管理**：创建新的 Telegraph 账号、查看账号信息、编辑账号设置、撤销并重新生成 access token
- **页面管理**：列出所有页面、创建新页面、编辑已有页面、软删除页面
- **令牌管理器**：在浏览器中保存并切换多组 Telegraph 账号（数据存于 `localStorage`），支持 JSON 文件导出/导入，方便备份或在不同端口之间迁移
- **单一可执行文件**：所有静态资源在编译时嵌入——部署时只需一个可执行文件
- **搜索与分页**：全文搜索所有页面，支持渐进式加载；分页式页面列表，可调整每页显示数量
- **批量操作**：勾选多个页面后一键删除（自动限速以遵守 Telegraph API 限制）
- **行内预览**：直接在界面中预览页面内容，无需另开 telegra.ph
- **多语言界面**：支持英文、繁体中文、简体中文；自动检测浏览器语言，也可从导航栏手动切换
- **深色模式**：自动跟随系统偏好切换深色/浅色主题，也可手动切换
- **零前端构建工具**：使用 HTMX 驱动交互，不需要 npm 或任何 JavaScript 框架

## 技术栈

| 组件 | 技术 |
|------|------|
| 后端框架 | [Axum](https://github.com/tokio-rs/axum) 0.8 |
| 模板引擎 | [MiniJinja](https://github.com/mitsuhiko/minijinja) 2 |
| 前端交互 | [HTMX](https://htmx.org/) 2（已内嵌） |
| HTTP 客户端 | [reqwest](https://github.com/seanmonstar/reqwest)（rustls） |
| 缓存/数据库 | [rusqlite](https://github.com/rusqlite/rusqlite)（内嵌 SQLite） |
| 静态资源嵌入 | [rust-embed](https://github.com/pyrossh/rust-embed) |

## 快速开始

### 前置要求

- [Rust](https://rustup.rs/)（1.85 及以上，支持 edition 2024）

### 构建与运行

```bash
# 克隆项目
git clone https://github.com/zeta987/telegraph-hub-rs.git
cd telegraph-hub-rs

# 构建并运行
cargo run

# 或构建 release 版本
cargo build --release
./target/release/telegraph-hub-rs
```

服务器默认启动于 `http://localhost:7890`。若该端口被占用，会自动尝试向上递增（最多尝试 10 个连续端口）。

### 环境变量配置

| 环境变量 | 默认值 | 说明 |
|----------|--------|------|
| `PORT` | `7890` | HTTP 服务器端口 |
| `RUST_LOG` | `telegraph_hub_rs=info` | 日志级别过滤器 |
| `TELEGRAPH_HUB_DB` | `telegraph_hub_cache.db` | SQLite 缓存数据库路径 |

## 使用方式

1. 在浏览器打开 `http://localhost:7890`
2. 创建新的 Telegraph 账号，或导入已有的 access token（令牌）
3. 从顶部下拉菜单选择要使用的令牌——页面列表会自动加载
4. 在页面列表中使用 **Edit** / **Delete** 按钮操作，或点击 **+ New Page** 创建新页面
5. 使用搜索栏**搜索**所有页面（首次使用时会建立服务端缓存）
6. **批量删除**：开启勾选模式，勾选多个页面后一键删除
7. **预览**：点击页面标题即可在界面内直接预览内容
8. **语言切换**：从导航栏的语言按钮切换 EN / 繁中 / 简中

### 令牌存储机制

Access token（令牌）保存在浏览器的 `localStorage` 中，按 origin（协议 + 主机 + 端口）隔离。服务器本身完全无状态，不会保存任何令牌。

由于 `localStorage` 按端口隔离，变更服务器端口后先前保存的令牌不会自动带过去。请利用 Saved Tokens 区域的 **Export** / **Import File** 按钮，将令牌导出为 `telegraph-hub-tokens.json` 文件，再于新端口导入即可。

## Telegraph API 支持范围

| 端点 | 状态 |
|------|------|
| `createAccount` | 已支持 |
| `editAccountInfo` | 已支持 |
| `getAccountInfo` | 已支持 |
| `revokeAccessToken` | 已支持 |
| `createPage` | 已支持 |
| `editPage` | 已支持 |
| `getPage` | 已支持 |
| `getPageList` | 已支持 |
| `getViews` | 已支持 |

## 开发指南

```bash
# 配合 cargo-watch 进行热重载开发
cargo watch -x run

# 静态分析
cargo clippy -- -D warnings

# 代码格式化
cargo fmt

# 执行测试
cargo test
```

## 许可证

[MIT](LICENSE)