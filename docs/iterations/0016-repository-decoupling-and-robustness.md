# 第十六阶段：仓库代码解耦与健壮性提升

## 状态

已验收。

开始日期：2026-07-26。

验收日期：2026-07-26。

## 目标

本阶段不增加新的邮件产品功能，也不改变已经验收的账户、同步、正文、通知、草稿和发件语义。目标是在现有单 Cargo package、ports 注入、Supervisor、SQLite Repository 与 TanStack Query 边界内完成三类工程改进：

1. 让 Rust 稳定错误、后台任务失败、前端未捕获异常和关键桌面副作用都进入同一份结构化日志。
2. 修复完整同步期间按需收取正文可能导致服务端断开同步连接的问题。
3. 系统拆分职责混杂或超过千行的实现文件，使协议、存储、运行时、Command 和测试代码更容易独立验证。

## 当前诊断

### 同步与正文请求的连接竞争

当前完整账户同步在 `AsyncImapProvider` 内同时建立 3 条 IMAP worker 会话；`MailRuntime::fetch_and_store_message` 在用户打开缺少正文的邮件时通过同一个 Provider 再建立 1 条独立会话。`MailRuntime.network_limit` 只限制“主动操作”数量，不知道每个操作内部实际建立了几条连接，因此一次同步与一次正文请求会让同一账户瞬时达到 4 条连接。

应用代码没有取消同步任务或复用同一 `Session`，因此正文请求不是在本地直接打断同步。更符合现有证据的原因是部分服务端或中间网络设备在达到并发连接限制后关闭已有连接，最终表现为同步路径 `ConnectionReset`。

### 大文件

阶段开始时主要超大文件包括：

- `storage/repository.rs`：约 2494 行，混合仓库装配、邮件读取、同步写入、数据库辅助函数和大量测试。
- `mail_runtime.rs`：约 1714 行，混合 Supervisor、邮件查询/操作、正文附件、同步执行、事件 DTO 与测试。
- `storage/draft_repository.rs`：约 1664 行，混合草稿与发件任务持久层及测试。
- `composer_runtime.rs`：约 1601 行，混合窗口、草稿、模板签名、附件、发件 Worker、辅助函数与测试。
- `storage/composition_definition_repository.rs`、`application/service.rs`、`commands/mod.rs`：均达到或超过约千行。

已有 `core/application/adapters/protocols/storage/runtime/commands` 依赖方向保持有效。本阶段只把现有职责移动到更窄的同层模块，不创建新 crate、不移动产品规则到错误层级。

## 实施范围

### 1. 全局诊断边界

- 在不让 `core` 依赖 `tracing` 的前提下，为所有 `CommandError` 构造安装进程级只读观察器，记录错误码、是否可重试以及 Rust 调用位置；不得记录 `params`、密码、Token、邮件正文、服务器原始响应或内部数据内容。
- 保留并增强阶段十五的 panic hook、IMAP/SQLx 原因链和前端 `window.error` / `unhandledrejection` 上报。
- 后台 Supervisor、发件 Worker、通知窗口、事件发布、恢复和清理路径不再静默吞掉非预期错误；可恢复失败记录结构化上下文后继续遵循原有状态机。
- 前端已捕获但此前直接忽略的基础设施失败，通过统一安全上报函数进入日志；正常的取消、窗口已关闭等预期分支不得制造错误风暴。

### 2. 账户级 IMAP 会话协调

- `ImapSyncProvider` port 和第三方协议类型边界保持不变；具体协调只存在于 `AsyncImapProvider` Adapter 内。
- 每个账户建立共享的会话预算，最多同时存在 3 条主动 IMAP 会话。
- 完整同步最多租用 2 条 worker 会话，始终为正文、附件、待办或草稿/Sent 操作保留 1 条交互会话容量。
- 每次操作按需建立连接，完成后正常 LOGOUT/关闭；失败连接不复用。预算使用弱引用按需回收，不引入常驻后台清理任务，也不把 `async-imap::Session` 越过 Provider 边界。
- 同一操作获取多条会话时必须有界并发；账户修改或重新认证继续由现有 Supervisor generation 和配置读取控制。
- 不引入 IDLE；IDLE 仍属于后续独立工作，且不得直接复用普通 60 秒静默读超时。

### 3. 模块拆分

- 将同步写入从综合 Repository 移至同层窄模块，将发件任务持久化从草稿 Repository 移至草稿子模块；邮件读取、草稿和既有正式测试继续使用相同共享 pool 与门面。
- 将 IMAP Provider、连接/登录、账户会话协调、文件夹同步与具体协议操作保持为独立模块。
- 将 MailRuntime 的正文/附件、持久化邮件操作、事件/观察器/Supervisor 支撑，以及 ComposerRuntime 的定义管理和发件调度从主运行时拆出。
- `repository.rs`、`draft_repository.rs`、`composition_definition_repository.rs` 和 `application/service.rs` 的总行数仍包含大段正式回归测试；生产职责已经低于或接近千行时保留测试邻近被测私有辅助函数，不为行数把测试变成跨模块公开 API。
- `commands/mod.rs` 继续作为约千行的稳定薄 IPC 清单；逐个函数只做 DTO 转发和少量宿主副作用，没有恢复业务构造或 SQLx/协议实现，因此本阶段不为目录外观改写 `generate_handler!` 表面。
- 前端现有 `mail/hooks`、统一 API、ErrorBoundary 和 Query/Event 边界已形成明确职责；本阶段只补诊断，不再次移动产品组件。

### 4. 实施后的主要文件边界

- `protocols/imap.rs`：文件夹同步和 UID/Flags 对账编排。
- `protocols/imap/{provider,connection,session_budget,session,parse,encoding,timeout}.rs`：具体 Provider、连接、有界容量、远端操作、解析、名称编码和超时。
- `storage/message_sync_repository.rs`：`MailSyncSink` SQLx 实现。
- `storage/draft_repository/send_job_repository.rs`：持久化发件任务。
- `mail_runtime/{content,operations,runtime_support}.rs`：正文附件、持久化待办、事件与 Supervisor 支撑。
- `composer_runtime/{definitions,delivery}.rs`：模板签名定义和持久化发件调度。

## 非目标

- 不实现 IMAP IDLE、OAuth、POP3、会话聚合、托盘或其他新产品能力。
- 不改变启动/1/5/10 分钟/手动完整同步时机。
- 不改变头部优先、打开时按需正文、逐封 `message-arrived`、通知基线和持久化待办语义。
- 不创建根 Cargo Workspace、业务子 crate、新数据库迁移或新的前端持久化。
- 不以重构为由放宽 HTML、TLS、凭据、账户槽或文件系统安全边界。

## 自动验证

- `pnpm test`
- `pnpm build`
- `cd src-tauri && cargo fmt --all -- --check`
- `cd src-tauri && cargo test --offline --locked`
- `cd src-tauri && cargo clippy --offline --locked --all-targets -- -D warnings`
- `git diff --check`

新增回归至少覆盖：

- 账户会话预算稳定复用同一协调器、上限为 3，完整同步只占 2 个 worker 配额。
- 同步持有两个会话时按需正文仍能获得第三个会话；第四个并发请求等待而不是再建连接或取消同步。
- `CommandError` 观察器不记录参数并能取得错误码、retryable 和调用位置。
- 拆分后现有 Repository、同步、草稿、发件、模板签名、HTML 和前端事件测试全部保持通过。

2026-07-26 自动验证结果：

- `pnpm test`：通过，29 个测试文件、80 项。
- `pnpm build`：通过；Vite 继续报告既有主入口与富文本 chunk 大于 500 kB 的非阻断警告。
- `cargo fmt --all -- --check`：通过。
- `cargo test --offline --locked`：通过，119 项 Rust 单元测试与全部 doc test。
- `cargo clippy --offline --locked --all-targets -- -D warnings`：通过。
- `git diff --check`：通过。
- 未运行 Tauri bundle。

## 实机验收门禁

Windows 10 22H2+：

1. 对大账户执行完整同步，同时连续打开缺少正文的邮件；正文可以下载，完整同步不得因新增连接被服务端重置或被应用取消。
2. 同步、正文下载、待办操作和发件并发时，收信按钮状态、逐封列表、已读状态和通知保持原有行为。
3. 人为触发一个前端错误、一个稳定 Command 错误和一个可恢复网络错误，确认都写入同一日志，且日志不包含密码、Token、正文或错误参数值。
4. 断网后恢复，超时与下一周期恢复语义不变，不出现亚秒级收信循环。
5. 常规阅读、附件、回复/转发、草稿、模板签名、通知和账户管理无重构回归。

完成自动验证后等待用户实机确认；未经确认不提交、不进入下一阶段。

## 验收结果

用户已确认第十六阶段实机验收通过。账户完整同步期间按需正文、既有邮件操作、日志诊断和重构后的常规工作流均满足本阶段门禁；本记录不扩大为未经单独执行的平台声明。
