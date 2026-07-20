# 0029 P1 Rust 分层与 IMAP 模块化重构

日期：2026-07-20

## 变更

- 将回复/回复全部/转发和服务器草稿导入的内容组合从 SQLite Repository 上移到纯 application 用例，并增加收件人去重、主题与线程规则单元测试。
- 将邮件存储拆为读取、同步写入、草稿、发件任务、待办操作和文件夹角色子仓库；运行时共享一个连接池与内容存储。
- 为账户配置、Bootstrap、外观与阅读偏好增加 core ports；为邮件运行时注入 IMAP、Repository Provider 与附件打开器，具体实现只在 Tauri 状态装配层出现。
- 统一 IMAP 连接/登录入口，将同步编排拆为文件夹、摘要、正文回填、Flags 对账和通知步骤，并将会话、MIME 解析、文件夹编码和同步策略拆入独立模块。
- 拆分邮件详情、Flags 排队与待办领取的长方法；简化普通远端操作和 Sent/Drafts APPEND 的 Worker 分派。

## 保持不变

- 公共 Tauri Command、DTO、事件载荷、稳定错误码、SQLite schema 与用户可见功能未改变。
- 协议类型、SQLx 类型、账户秘密与内部路径仍不会暴露给 React。
- Rust 保持单一 `src-tauri` Cargo package；未新增依赖、根 Cargo Workspace 或第二套构建目录。

## 验证

- `cargo fmt --all -- --check` 通过。
- `cargo test --offline --locked`：58 项测试全部通过。
- `cargo clippy --offline --locked --all-targets -- -D warnings` 通过。
- `pnpm test`：14 个测试文件、29 项测试全部通过。
- `pnpm build` 通过；主入口 chunk 仍有大于 500 kB 的既有提示，留待前端结构批次。
- `git diff --check` 通过。
- 未执行 Tauri bundle；正常增量构建产物按约定保留。

## 手动验收

- 2026-07-20：用户完成 Windows 实机验证并确认通过。
