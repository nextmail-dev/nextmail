# 第八阶段：架构与性能重构

状态：P0 与 P1 Rust 分层批次均已通过手动验收；下一批为 P1 前端结构重构。

## 目标

以 `docs/refactor-suggestions.md` 的审查结果为输入，在不增加产品功能、不改变稳定 Command DTO、错误码和安全边界的前提下，分批修复正确性问题、同步与存储热路径、分层泄漏、前端状态重复和工具链缺口。

本阶段仍遵循单一 `src-tauri` Cargo package，不恢复根 Cargo Workspace。每一批完成自动验证后单独交付手动验收；未经确认不进入下一批，也不提交。

## 实施批次

1. **P0 正确性与热路径**：F1、R1–R4。
2. **P1 Rust 分层**：R5–R13，先上移回复/转发用例，再拆 Repository、注入 ports，最后拆 IMAP 编排。
3. **P1 前端结构**：F2–F4，合并外观偏好数据源，并拆分主工作区状态、事件与分栏 hook。
4. **P2/P3 性能与清理**：F5–F12、R14–R19、T1–T2；工具链是否加入远程 CI 仍受“不配置远程仓库”约束，本地 lint/format 可独立实施。

## 第一批实施结果

### 前端查询刷新

- 新增邮件详情 query-key 工厂，详情查询始终使用 `accountId/mailboxId/messageId` 四段 key。
- `message-content-changed` 不包含 mailbox ID，因此事件监听按 `accountId` 两段前缀失效全部相关详情。
- 附件下载已经使用正确的完整 key，本批保留更精确的失效范围，并新增组件回归测试。

### SQLite 原子写入与对账

- 邮件、位置、正文和附件元数据在同一 SQLx 事务内写入；任一附件语句失败时不留下半成品行。
- 原始 EML 是内容寻址文件，事务开始前写入，避免持有 SQLite 锁等待文件 I/O；数据库失败最多留下无引用哈希内容，不会形成可见邮件半成品。
- 附件元数据按 100 条多行 UPSERT，避免逐附件数据库往返并控制 SQLite bind 参数数量。
- 文件夹对账使用事务连接上的临时远端 UID 集合，再以单条集合 DELETE 移除服务端已不存在且没有活动待办的位置，消除逐行 COUNT/DELETE。

### TLS 与 IMAP 正文热路径

- IMAP 真实同步和首次账户连接测试复用同一进程级 rustls `ClientConfig`，系统根证书只在首次需要 TLS 时加载一次。
- 新邮件摘要仍按 100 封拉取；需同步正文的 UID 在该批内合并为一次 `UID FETCH ... BODY.PEEK[]`。
- 旧的缺失正文回填也按 100 封批量拉取，并保持 UIDVALIDITY、缺失正文错误和同步进度语义。

## 自动验收

- 前端：邮件附件下载完成后，详情 query 使用精确四段 key 失效；后台正文事件使用账户级前缀。
- 存储：多附件批量写入成功；强制附件 INSERT 失败时四张相关表均无残留；reconcile 删除无待办位置并保留活动待办位置。
- TLS：相同缓存重复取得同一 `Arc<ClientConfig>`，加载器只调用一次。
- IMAP：UID 批次格式测试覆盖单次 FETCH 的 UID 集合构造。
- 完整的 TypeScript、Vitest、Rust test、fmt 与 Clippy 结果写入 `changes/0028-refactor-p0-hardening.md`。

## 手动验收

1. 启动已有账户，确认本地邮件立即出现，后台正文下载完成后已打开的阅读器自动刷新。
2. 下载一份尚未本地缓存的附件，确认状态更新并能按当前偏好打开。
3. 手动收取一个包含多封新邮件和多附件的文件夹，确认摘要与正文逐步出现，无重复邮件或同步卡死。
4. 断网后恢复，确认待办操作和下一轮对账仍正常。
5. Windows 实机确认首次 TLS 连接、后续同步和账户验证均可用；macOS 由后续环境补充，不宣称未经执行的平台通过。

验收结果：2026-07-20 用户实机验收通过，进入 P1 Rust 分层批次。

## 第二批实施结果

### 应用、存储与装配边界

- 回复、回复全部、转发和服务器草稿导入的内容编排移入 `application/message_composer.rs`，收件人去重、主题前缀、引用块和 Tiptap/HTML 生成可脱离 SQLite 纯测试；Repository 只读取源数据和持久化结果。
- 原 `MailRepository` 拆为读取、同步写入、草稿、发件任务、待办操作与文件夹角色子仓库；共享门面只负责打开同一个连接池、内容存储并提供窄访问器。
- Bootstrap、账户、外观和阅读偏好通过 `core::ports` 注入 `AppService`；IMAP Provider、Repository Provider 与附件打开器通过运行时构造器注入，具体 Adapter 统一在 `state.rs` 组合。
- 写信运行时复用邮件运行时已经打开的 Repository，不再为同一数据目录建立第二个连接池。

### IMAP 与待办操作

- IMAP 的 TLS/STARTTLS/明文连接与登录收敛为单一路径，所有协议操作复用统一会话建立逻辑。
- 文件夹同步主循环拆为文件夹编排、摘要批次、正文回填、Flags 对账和事件通知；核心循环可一屏阅读。
- 原单文件按职责拆为 `session.rs`、`parse.rs`、`encoding.rs` 与 `policy.rs`，协议类型仍不越过 Adapter 边界。
- 邮件详情查询使用 `FromRow` 投影与独立查询/映射函数；Flags 排队拆出单项事务、乐观投影和按类型更新；待办领取拆开查询、原子状态转换和工作项映射。
- 待办 Worker 把普通远端操作、Sent APPEND 与 Draft APPEND 分开处理，复用目的文件夹和 MIME 载入校验，保留既有重试与恢复语义。

## 第二批自动验收

- `cargo fmt --all -- --check` 通过。
- `cargo test --offline --locked`：58 项测试全部通过。
- `cargo clippy --offline --locked --all-targets -- -D warnings` 通过。
- `pnpm test`：14 个测试文件、29 项测试全部通过。
- `pnpm build`：TypeScript 与 Vite 生产构建通过；既有主入口 chunk 大小提示保留给后续前端拆分批次。
- `git diff --check` 通过；未执行 Tauri bundle，`dist` 与 `src-tauri/target` 继续作为增量构建缓存保留。

## 第二批手动验收

1. 启动多个现有账户，确认本地邮件立即显示、后台同步继续渐进落库，账户切换不串数据。
2. 分别执行回复、回复全部和转发，确认收件人、主题、引用正文、线程头和转发附件与重构前一致。
3. 新建草稿并关闭窗口，确认本地保存与 Drafts 同步正常；发送一封邮件，确认 SMTP 只发送一次且 Sent 归档正常。
4. 执行已读、星标、移动、复制和删除，再断网重启恢复，确认本地乐观状态、待办重放与失败重试正常。
5. 手动收取并保持 Inbox 一段时间，确认 TLS/STARTTLS 连接、IDLE/轮询、文件夹同步和正文解析无回归。

验收结果：2026-07-20 用户实机验收通过，P1 Rust 分层批次完成。
