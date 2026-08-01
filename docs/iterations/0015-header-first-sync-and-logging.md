# 第十五阶段：头部优先同步与全局日志诊断

## 状态

已验收。

验收日期：2026-07-26。

本阶段围绕同步稳定性与可诊断性，处理三件事：移除"正文同步范围/始终下载正文"可配置项、建立全局日志与异常捕获、将同步改为"头部优先 + 打开时按需下载正文"。起因是用户在收取过程中观察到 `sync.message_fetch_failed`（Windows 10054 ConnectionReset）且同步按钮卡在 loading、无法手动重启。

## 诊断：同步失败与按钮卡死

- **10054 ConnectionReset**：远端邮件服务器或中间网络设备在 FETCH 期间强制关闭了 TCP 连接。`FETCH_BATCH_SIZE = 1` 使单次同步在同一长连接上对每封邮件发出 2 条 FETCH（头部+正文），命令数随邮件数线性增长，既可能触发部分服务器的会话/速率限制，也拉长了连接存活时间、放大了被重置的概率。
- **按钮卡死、无法手动启动**：按钮 loading 以账户运行状态 `syncing` 为准（`MainShell.receiving`），`sync_now` 在 `Syncing` 时返回 `sync.already_running`。IMAP TCP 会话（`connect_session`）未设置读/连接超时，当连接进入半开或服务器静默时，同步任务会无限挂起，持有 `Syncing` 状态与网络信号量，状态无法迁出、手动收取被拒。日志中的 ConnectionReset 是一次"干净"失败（状态会迁到 Offline），卡死更可能来自其后的周期重试在静默连接上挂起。本阶段已通过 IMAP 读/写超时修复（见下）。
- **头部优先的直接帮助**：头部先落库、正文改为按需拉取，同步期数据量与往返大幅减少、被重置概率降低；下次同步跳过已同步头部继续。

## 范围与结果

### 同步策略简化（前序）

- 删除 `SyncPolicy` 枚举与 `download_non_inbox_bodies` 选项（核心类型、持久层 get/set、Tauri 命令、前端 UI 与中英文案），默认同步服务器全部消息。`SyncInterval` 保留。
- 删除 `protocols/imap/policy.rs` 时间窗口逻辑。`account_sync_settings` 的 `sync_policy` / `download_non_inbox_bodies` 列保留不删，避免迁移风险、不再读写。

### 全局日志与异常捕获

- 新增 `tracing` + `tracing-subscriber` + `tracing-appender`。`logging.rs` 在 `setup` 最先初始化：按天滚动写入 `<app_local_data_dir>/logs/nextmail.log.YYYY-MM-DD`，默认 `info`、可用 `RUST_LOG` 覆盖，并安装 panic hook。
- IMAP 同步路径所有 `.map_err(|_| ...)` 吞错点改用 `map_imap_err`，记录真实底层错误后再转 `CommandError`；`run_sync` 记录开始/完成/失败。此前"同步失败"只带错误码、原因被丢弃，现在日志含真实 io/imap 原因（如本次的 `Io(ConnectionReset)`）。
- 前端 `errorReporting.ts` 注册 `window.error` / `unhandledrejection`，经新增 `log_frontend_event` 命令写入同一日志；`main.tsx` 启动时挂载。React 错误边界仍保留。

### 头部优先同步（本阶段核心）

- `fetch_summaries` 只抓 `BODY.PEEK[HEADER]` 并以 `raw: None` 入库（`body_availability = missing`、预览为空、附件数暂为 0），逐封发 `message-arrived` 让列表靠头部先一封封出现。
- 同步不再自动下载正文：移除 `backfill_bodies` / `backfill_worker` 与 `BACKFILL_BATCH_SIZE`、`fetch_remote_messages` 导入。正文改为用户点开邮件时按需拉取（`request_message_body`），大幅减少同步期数据量与往返、降低被重置概率，也免去预览回填的额外刷新。
- 前端 `MessageViewer` 打开无正文邮件时自动触发正文拉取（带真实进度），不再常驻"下载正文"按钮；仅在拉取失败时显示重试按钮。
- `FETCH_BATCH_SIZE = 1` 保留以维持头部逐封到达；删除仅头部阶段不再使用的 `fetch_raw_batch` 与 `HashMap` 导入。

### IMAP 超时与自动恢复

- 新增 `protocols/imap/timeout.rs` 的 `TimeoutStream`：在传输层为每个读/写附加超时预算（`IMAP_IO_TIMEOUT = 60s`），每次成功收发数据后重置；只有真正静默的连接才超时，正在流式传输的大正文不会误判。
- `connect_session` 的 TCP 连接另加 `IMAP_CONNECT_TIMEOUT = 30s`。
- 超时产生的 `TimedOut` io 错误经 `map_imap_err` 记录并转为可重试 `CommandError`，沿 `run_sync` 失败分支把账户置为 `Offline`，由既有按间隔的周期同步自动重试——即"卡死后自动恢复"，无需新增重试逻辑。
- 覆盖所有 IMAP 连接路径（同步、backfill、按需拉正文、待办操作）。

### 文件夹内并发抓取

- `synchronize` 用 `try_join_all` 并发建立 `SYNC_WORKER_COUNT = 3` 个 IMAP 会话，跨文件夹复用以摊销登录成本。
- `sync_folder` 让每个 worker 会话进入邮箱，把待抓 UID 按连续区间切成 3 份不相交子集，用 `join_all` 并发抓取头部；`reconcile_flags` 仍走单会话。新增 `split_uids` 切片辅助与 `fetch_summaries_worker`。
- 进度计数用 `AtomicU64` 跨 worker 聚合；`sink`/`observer` 均为 `Send + Sync`（内部加锁），并发 `notify` 与 `upsert` 安全。
- worker 数选 3：兼顾"2-4 个"诉求，且不超过 SQLite 连接池上限（4）；单账号 3 条 IMAP 连接在多数服务器并发限额内，账户级并发上限（`network_limit = 2`）不变。
- 修复 `split_uids` 在空 UID 列表（文件夹无新邮件）时 `slice::chunks(0)` panic 的 bug（`chunk_size` 至少为 1），补空输入回归测试。

### 同步进度 UI

- 同步进行时在文件夹面板显示 `已同步 completed/total`（如 `· 5/50`），复用既有 `SyncProgress.completed/total`，按阶段反映文件夹/头部/正文进度。
- 移除"邮件同步失败"告警条；同步错误已落入日志文件，不再打扰 UI。邮箱列表读取错误提示保留（与同步错误无关）。

### 大文件夹列表性能

- 修复同步进行时 UI 越来越卡（切换到正在同步的文件夹时尤甚）：`message-arrived` 增量插入曾让第一页无界增长（6000+ 邮件全部渲染），且 3 个 worker 高频事件（~60/秒）每次触发整表重渲染，呈 O(n²)，200 封起即明显卡顿。
- `applyArrivedMessage` 将第一页**封顶 50**：超出时裁剪并把被挤出的项交给"加载更多"按游标重新拉取，列表恒为 50 行、不随同步进度增长。
- `message-arrived` 改为**节流**：选中邮箱的到达事件缓冲后每 100ms 合并 flush 一次（单次 `setQueryData` 只触发一次重渲染），刷新频率从 ~60/秒降到 ~10/秒；非选中邮箱仍即时 invalidate。
- `MainShell` 的 `visibleMessageIds` 由 state 改为 ref：它只用于删除后选区计算，无需触发渲染；原先每次到达都 `setState` 导致 MainShell（及沉重的 MessageViewer）跟着重渲染，是"整个 UI 都卡"的放大器。

### 存储写错可诊断化、写锁串行与断点续传

- 现象：6000+ 大文件夹同步反复以 `storage.message_write_failed` 失败（非 IMAP 错误），`complete_mailbox` 从未执行 → `last_uid` 停在 0 → 每次重试都从 1 重抓已入库邮件（幂等重写、重新计数），永远跑不完、下次仍从 1 开始。
- 定位：持久层 `upsert_message` 等大量 `.map_err(|_| CommandError::new("storage.*"))` 吞掉 sqlx 真实错误，日志只见错误码、不见原因。SQLite 即便 WAL 也只允许一个写事务，3 个 worker 各开写事务、又未设 `busy_timeout`，抢锁失败者立即 `SQLITE_BUSY` → 写"失败" → 同步中止。与引入多 worker 直接相关。
- 修复：
  - **可诊断化**：新增 `map_storage_err`（仿 `map_imap_err`），45 处 `storage.*` 吞错点与 `storage_read_error` 改为 `tracing::warn!` 记录真实 sqlx 错误后再转 `CommandError`，日志可见 `database is locked`/磁盘 I/O/触发器错误等真实原因。
  - **写锁串行**：`sync_folder` 引入 `tokio::sync::Mutex`，worker 仅在 `sink.upsert_message` 期间持锁，3 个 worker 的 DB 写串行、不再抢锁；IMAP 头部抓取仍各连接并行（网络才是瓶颈）。`open_pool` 另显式 `busy_timeout(15s)` 作为跨来源（正文按需拉取、前端触发写）争用的兜底。
  - **断点续传**：新增 `MailSyncSink::stored_uids(mailbox_id, uid_validity)`，`sync_folder` 改为按"远端 UID − 已入库 UID"差集只抓缺失部分。中途失败（网络/超时等）下次自动从断点续抓，不再从 1 重来。差集法在连续分块下也正确：中间 worker 失败留下的空洞会在下次被补上，而不会像高水位那样被永久跳过。`last_uid` 高水位保留（仍由 `complete_mailbox` 写入、记 `last_synced_at`），但抓取决策以差集为准。

### 已读状态回退修复

- 现象：正文未同步的邮件，点开后正文加载显示，再切到下一封时，刚才那封的已读状态又变回未读，需再次点击才能标记已读。
- 根因：点开邮件时 `MessageListPane` 触发 `set_message_read`（本地立即置已读 + 排队 pending `set_read`），同时 `MessageViewer` 自动拉正文；`fetch_and_store_message` 用服务器返回的（仍为未读的）flag 调 `upsert_message`，其 `ON CONFLICT` 无条件 `unread = excluded.unread`，把本地已读覆盖回未读。
- 修复：`upsert_message` 的 `message_locations ON CONFLICT` 对 `unread`/`flagged` 加 `pending_operations` 守卫（与 `reconcile_mailbox` 一致）--存在未完成的 `set_read`/`set_flagged` 操作时保留本地值，不被服务器 stale flag 覆盖。补回归测试 `upsert_message_preserves_pending_read_flag_against_stale_server_state`。

### 待办操作 STORE 修复与失败可诊断化

- 现象：阅读邮件后服务器端未标记已读，本地待办队列堆积，UI 反复报"无法更新状态"，日志却无任何错误。修顺序后仍有部分邮件失败。
- 根因：`set_read`/`set_flagged` 走 CONDSTORE `UNCHANGEDSINCE` 条件 STORE，三重隐患--① `conditional_store_query` 生成 `(UNCHANGEDSINCE N) +FLAGS (\Seen)`，token 顺序违反 RFC 4551（modifier 应在 flag 列表后），CONDSTORE 服务器判 BAD；② 即便顺序正确，冲突时 `UNCHANGEDSINCE` 失败仍返回 UID（flag 未应用），`update.uid.is_some()` 误判成功，app 以为已读而服务器未改；③ 部分服务器声明 CONDSTORE 却拒绝 `UNCHANGEDSINCE` modifier。叠加 `apply_operation_session` 与 `drain_pending_operations` 用 `|_|` 吞掉真实 imap 错误且不记日志，故日志无体现。
- 修复：
  - `set_read`/`set_flagged` 改为**无条件 STORE**（`+FLAGS (\Seen)`），移除 CONDSTORE `UNCHANGEDSINCE` 条件与重试、删除 `conditional_store_query` 及其单测。`+FLAGS`/`-FLAGS` 只改命名 flag、不覆盖其它 flag，无丢失更新风险，且总是生效，一次解决上述三重隐患。未触及任何邮件（uid 已被 expunge）时 `tracing::warn!` 记录后返回 `operation.store_failed`。
  - 新增 `map_operation_err`（仿 `map_imap_err`），把 `session.rs` 中 30 处 `|_| CommandError::retryable(...)` 吞错点改为 `tracing::warn!` 记录真实 imap 错误后再转 `CommandError`。
  - `drain_pending_operations` 失败分支补 `tracing::warn!`（kind/uid/mailbox/attempt/code/retryable），待办失败既进 UI 也进日志。
  - **最终细化（.SILENT）**：非 SILENT 的 `+FLAGS` 以 `update.uid.is_some()`（FETCH 响应是否带回 UID）判定成功，部分服务器对 `+FLAGS` 不回 FETCH--STORE 实际成功（web 端已标记已读）却判为 `operation.store_failed`、无限重试、队列卡死（日志 `STORE touched no message`）。改为 `+FLAGS.SILENT (\Seen)`，以**命令成功**（无错误）作为已应用，不再依赖 FETCH 响应；存在性已由前置 UID SEARCH 确认。

## 安全与架构边界

- 事件仍只负责本地 Query 失效/重读或 `setQueryData` 增量插入，不推送正文；头部与正文仍各自走原子 upsert，不在网络等待期持有 SQLite 写锁。
- 日志写入 app 本地数据目录下的 `logs/`，与可配置数据目录解耦；不记录凭据、不触网络。前端上报仅 level/message/location，不含敏感结构。
- 不新增 Tauri bundle；仅新增 `tracing` 系日志依赖与 `log_frontend_event` 命令。DB 列保留、无新迁移。
- 并发仅限单文件夹内、worker 固定 3，不改变账户级并发上限（`network_limit = 2`）；worker 间共享状态均为 `Send + Sync` 或原子计数。

## 自动验证

- `cd src-tauri && cargo fmt --all -- --check`：通过。
- `cd src-tauri && cargo test --lib`：通过，116 项。
- `cd src-tauri && cargo clippy --all-targets -- -D warnings`：通过。
- `pnpm tsc --noEmit`：通过。
- `pnpm test`：通过，29 个测试文件、80 项。

## 待办与后续

- **可选后台正文预取**：本阶段交付时没有后台正文 backfill，正文只在用户打开邮件时按需下载。第二十一阶段后来增加默认关闭的账户全文同步开关：仍先完成逐封头部落库，再复用既有两条同步会话补齐缺失正文，并保留第三条交互容量；没有重新阻塞头部同步或增加调度入口。
- **批次调优**：`FETCH_BATCH_SIZE = 1` 命令数多、重置概率高；由于 `message-arrived` + `setQueryData` 已保证逐封出现，头部批次可适度上调以减少往返，待实机验证后决定。
- **日志清理**：`tracing-appender` 按天滚动但不自动删除旧文件，长期积累，可加"保留最近 N 天"的清理。

## 手动验收门禁

Windows 10 22H2+：

1. 新账户初始同步：列表应靠头部一封封快速出现（暂无预览）；点开任一邮件应自动拉取正文并显示进度，完成后展示正文/预览，无需手动点下载按钮。
2. 同步中触发网络重置（如断网/恢复）：日志记录真实 io 错误；恢复网络后下次同步跳过已落库头部、继续补正文，不重复下载已存邮件。
3. 查看 `%LOCALAPPDATA%\com.taurusxin.nextmail\logs\nextmail.log.*`，确认同步开始/完成/失败与 imap 底层错误均落盘。
4. 前端触发未捕获异常（如控制台手动 `throw`）确认写入同一日志。
5. 同步中模拟连接静默（如挂起网络）：60s 内同步应失败、按钮恢复，并在下个周期自动重试；不再永久卡在 loading。
6. 同步 6000+ 大文件夹：写锁串行后多 worker 不再 database is locked，同步应能一次跑完；日志若仍有存储错误应带真实 sqlx 原因。即便中途因网络/超时失败，再次同步也应只抓缺失部分（计数从断点继续、不归零重抓）。
7. 打开一封正文未同步的未读邮件：正文自动加载显示、邮件标记为已读；切换到下一封后再切回，前一封应仍为已读（不回退为未读、无需再点一次）。
8. 阅读若干未读邮件后到 web 端确认已标记已读；本地待办队列应排空、不再报"无法更新状态"。若仍有失败，日志应含 `imap operation failed`（真实 imap 原因）与 `pending operation failed`（kind/uid/code）两条。

## 验收结果

用户于 2026-07-26 明确确认第十五阶段验收通过。后续结构与健壮性工作进入第十六阶段，不回改本阶段已验收的同步产品语义。

## 迭代变更摘要

- 0051：实现头部优先同步、按需正文、统一日志、IMAP 超时、并发抓取、断点续传和待办修正。
