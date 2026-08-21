# HTML-only 邮件正文文本提取

状态：等待手动验收

## 范围

- HTML-only 邮件生成纯文本与列表预览时，只从 `<body>` 提取可见文本，不再包含 `<head>` 中的 `<title>`。
- 完整原始邮件与按 MIME section 获取正文两条解析路径保持一致。
- HTML 安全清洗明确丢弃 `<title>` 内容，避免标题被扁平化进阅读正文。

## 非目标

- 不改变 `text/plain` 邮件及 multipart/alternative 中真实纯文本部分的优先级。
- 不改变 HTML/CSS 白名单、远程图片、CID、iframe sandbox 或正文选择性下载策略。
- 不引入新的 HTML 解析依赖。

## 验证门禁

- `src-tauri` 内 `cargo fmt --all -- --check`、相关定向测试、`cargo test --offline --locked`、`cargo clippy --offline --locked --all-targets -- -D warnings`。
- `git diff --check`。

## 实施与验收记录

- `src-tauri/src/protocols/html.rs`：新增共享的 HTML-only 正文文本提取，保留真实 `text/plain` 优先级；存在 `<body>` 时只将其内容交给 `mail-parser` 的既有 HTML→文本转换。HTML 清洗同时把 `<title>` 作为整段丢弃的不可见内容，避免清洗后进入阅读正文。
- `src-tauri/src/protocols/imap/parse.rs`：完整 EML 解析的 `plain_text` 与 `preview` 复用共享规则。
- `src-tauri/src/protocols/imap/structure.rs`、`session.rs`：按 MIME section 获取 HTML-only 正文时同步生成 body-only 纯文本与预览，和完整拉取路径一致。
- 回归测试覆盖完整 HTML 文档的 `<head><title>` 不进入纯文本、预览及安全 HTML，以及选择性 HTML section 路径只提取 `<body>`。
- 自动验证：`cargo fmt --all -- --check`、两项定向测试、`cargo test --offline --locked`（189 passed）、`cargo clippy --offline --locked --all-targets -- -D warnings`、`git diff --check` 全部通过。
- 待用户 Windows 实机验收：打开一封仅含完整 HTML、没有独立 `text/plain` 的邮件，确认列表摘要与阅读正文均不显示 `<head>` 中的标题，只显示 `<body>` 内容。
