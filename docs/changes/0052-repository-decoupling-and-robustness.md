# 0052：仓库解耦、全局诊断与账户 IMAP 会话协调

日期：2026-07-26

状态：已验收（2026-07-26）。

## 变更

- 为 `CommandError` 增加不依赖 `tracing` 的进程级观察钩子。宿主日志层记录所有稳定错误构造的 code、retryable 和 Rust 调用位置，明确不接收 `params`，避免用户输入或敏感数据进入诊断。
- 前端错误边界、`window.error`、`unhandledrejection` 和关键异步监听失败统一通过 `log_frontend_event` 写入现有按日滚动日志；上报长度由 Command 限制。后台 Supervisor、发件 Worker、通知窗口、关键 Event、窗口副作用和凭据补偿路径不再静默吞掉非预期错误。
- `AsyncImapProvider` 新增账户级有界会话预算：每个账户最多三条主动 IMAP 会话，完整同步从三 worker 收敛为两 worker，并保留第三条给按需正文、附件和持久化远端操作。第四个请求等待租约，不建立额外连接或取消同步；操作结束后连接照常关闭。
- IMAP 实现拆为 `connection.rs`、`provider.rs`、`session_budget.rs`、`session.rs`、`parse.rs`、`encoding.rs`、`timeout.rs` 与保留在 `imap.rs` 的文件夹同步编排。第三方 `Session` 仍不越过 protocol Adapter。
- `MailSyncSink for SyncSinkRepository` 从综合 `storage/repository.rs` 移至 `storage/message_sync_repository.rs`；草稿与发件任务持久化分别位于 `draft_repository.rs` 和 `draft_repository/send_job_repository.rs`。共享 SQLx pool、事务、账户槽和内容存储语义不变。
- `MailRuntime` 的正文/附件、持久化邮件操作、事件/Observer/Supervisor 支撑分别移至 `mail_runtime/content.rs`、`operations.rs`、`runtime_support.rs`；主文件从阶段开始时约 1714 行降至约 900 行。
- `ComposerRuntime` 的模板签名定义命令与持久化发件调度分别移至 `composer_runtime/definitions.rs`、`delivery.rs`；主文件从约 1601 行降至约 1000 行。
- 拆分仅使用同一 Cargo package 内的同层子模块和受控 `pub(super)` 可见性。稳定 ports、DTO、Command 名称、数据库 schema、同步时机、草稿/发件状态机及前端 Query/Event 契约均未改变。
- 同步技术文档纠正阶段十五后仍残留的旧正文时间/非收件箱正文策略、三 worker 正文回填和条件 STORE 描述；新增 ADR 0014 记录有界账户会话的长期理由。

## 安全与架构边界

- 不新增依赖、Capability、数据库迁移、Cargo package 或前端持久化。
- 日志不记录密码、Token、邮件正文、`CommandError.params` 或服务器原始响应；协议/SQLx 具体错误只留在本地诊断文件，不进入稳定 IPC DTO。
- 会话预算按公开账户 ID 协调，但数据访问仍先解析为匿名 `account_slot_id`；预算中不保存凭据或邮件数据。
- IMAP TLS、严格系统信任链、明文确认和 60 秒普通读写超时保持不变；未实现 IDLE。

## 验证

- `pnpm test`：通过，29 个测试文件、80 项。
- `pnpm build`：通过；仅保留既有大 chunk 非阻断警告。
- `cargo fmt --all -- --check`：通过。
- `cargo test --offline --locked`：通过，119 项 Rust 单元测试与全部 doc test。
- `cargo clippy --offline --locked --all-targets -- -D warnings`：通过。
- `git diff --check`：通过。

用户已确认实机验收通过：同步期间按需正文、日志覆盖和常规邮件回归符合阶段门禁。本记录不宣称未经单独执行的平台结果。
