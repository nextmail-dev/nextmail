# 0051：头部优先同步、全局日志与 IMAP 超时

日期：2026-07-24

状态：已验收（2026-07-26）。

## 变更

- 删除"正文同步范围"（30/90/365 天/全部）与"始终下载非收件箱正文"可配置项，默认同步服务器全部消息；`SyncInterval`（手动/1/5/10 分钟）保留。数据库列保留不删，避免迁移风险。
- 同步改为仅头部：`fetch_summaries` 只抓头部并逐封入库、发 `message-arrived`，列表靠头部一封封出现（暂无预览）；同步不再自动下载正文（移除 `backfill_bodies`），改为用户点开邮件时按需拉取（`request_message_body`）。大幅减少同步期数据量与往返、降低被重置概率。前端打开无正文邮件自动拉取（带进度），不再常驻"下载正文"按钮，仅失败时显示重试。
- 新增全局日志：`tracing` + `tracing-appender` 按天滚动写入 `<app_local_data_dir>/logs/nextmail.log.YYYY-MM-DD`（默认 `info`，`RUST_LOG` 可调），含 panic hook。IMAP 同步路径所有 `.map_err(|_| ...)` 吞错点改用 `map_imap_err` 记录真实底层错误；`run_sync` 记录开始/完成/失败。此前"同步失败"只带错误码、原因被丢弃。
- 前端注册 `window.error` / `unhandledrejection`，经新增 `log_frontend_event` 命令写入同一日志；React 错误边界保留。
- IMAP 传输层加 `TimeoutStream`：每个读/写 60s 超时（每次成功收发后重置，流式传输的大正文不会误判），TCP 连接 30s 超时。静默/半开连接在有限时间内失败并转 `Offline`，由既有按间隔的周期同步自动重试，不再永久卡在 loading、无法手动重启。
- 单文件夹内并发抓取：`synchronize` 并发建立 3 个 IMAP 会话（跨文件夹复用），`sync_folder` 把待抓 UID 切成 3 份不相交子集，`join_all` 并发抓取头部与正文；`reconcile_flags` 仍单会话。worker 数 3 兼顾性能与 SQLite 连接池（4）/服务器并发限额。
- 同步进度 UI：同步进行时显示 `已同步 completed/total`（如 `· 5/50`）；移除"邮件同步失败"告警条，同步错误只入日志、不打扰 UI。
- 大文件夹列表性能：`message-arrived` 增量插入将第一页封顶 50（超出按游标交给"加载更多"），并把选中邮箱的到达事件节流为每 100ms 合并一次 `setQueryData`；修复同步 6000+ 邮件文件夹时 UI 越来越卡（列表无界增长 + 高频整表重渲染）。
- 存储写错可诊断化、写锁串行与断点续传（补充，2026-07-25）：持久层 45 处 .map_err(|_| ...) 吞错点改用新增 map_storage_err 记录真实 sqlx 错误（storage_read_error 同改），实机日志确认为 database is locked（SQLITE_BUSY）。sync_folder 引入 tokio::sync::Mutex 串行 worker 的 DB 写、IMAP 抓取仍并行，open_pool 另加 busy_timeout(15s) 兜底，根治多 worker 并发写 SQLite 时 database is locked 导致大文件夹同步反复 storage.message_write_failed 的问题。新增 MailSyncSink::stored_uids，同步改为按"远端 UID 减已入库 UID"差集只抓缺失部分，中途失败下次从断点续抓、不再从 1 重来。

- 已读状态回退修复（补充，2026-07-25）：打开正文未同步的未读邮件时，本地已标记已读并排队 pending set_read，但 `fetch_and_store_message` 用服务器 stale flag 调 `upsert_message` 又把已读覆盖回未读（切到下一封后回退为未读）。给 `upsert_message` 的 `message_locations ON CONFLICT` 的 unread/flagged 加 pending_operations 守卫（与 `reconcile_mailbox` 一致），存在未完成 set_read/set_flagged 时保留本地值。补回归测试。

- 待办操作 STORE 修复 + 失败可诊断化（补充，2026-07-25）：`set_read`/`set_flagged` 走 CONDSTORE `UNCHANGEDSINCE` 条件 STORE 有三重隐患--token 顺序违反 RFC 4551（`(UNCHANGEDSINCE N) +FLAGS`，服务器判 BAD）、冲突时静默 no-op（失败仍返回 UID，app 误判成功而服务器未改）、部分服务器声明 CONDSTORE 却拒绝 `UNCHANGEDSINCE`。改为**无条件 STORE**（`+FLAGS (\Seen)`，只改命名 flag、不覆盖其它 flag，无丢失更新且总是生效），删除 `conditional_store_query` 及其单测。同时新增 `map_operation_err`，把 `session.rs` 30 处 `|_|` 吞错点改为记录真实 imap 错误；`drain_pending_operations` 与 `store_flag_delta` 未触邮件分支补 warn 日志（此前失败只进 UI、日志无体现）。**最终改为 `.SILENT`**：非 SILENT `+FLAGS` 以 FETCH 响应判定成功，部分服务器对 `+FLAGS` 不回 FETCH，STORE 实际成功（web 已标记）却判 `operation.store_failed`、队列卡死；改用 `+FLAGS.SILENT (\Seen)` 并以命令成功为准、不再依赖 FETCH 响应。

## 边界

- 事件仍只负责本地 Query 失效/重读或 `setQueryData` 增量插入，不推送正文；头部与正文各自原子落库，不在网络等待期持有 SQLite 写锁。
- 日志写入 app 本地数据目录下的 `logs/`，与可配置数据目录解耦；不记录凭据、不触网络。前端上报仅 level/message/location。
- 超时为传输层每读/写预算，只在连接真正静默时触发；不引入新的重试调度，复用既有按间隔周期同步实现"自动恢复"。
- 并发仅限单文件夹内、worker 固定 3，不改变账户级并发上限（`network_limit = 2`）；worker 间共享状态均为 `Send + Sync` 或原子计数。
- 仅新增 `tracing` 系日志依赖与 `log_frontend_event` 命令；DB 列保留、无新迁移、不新增 Tauri bundle 或 Capability。

## 验证

- `cd src-tauri && cargo fmt --all -- --check`：通过。
- `cd src-tauri && cargo clippy --all-targets -- -D warnings`：通过。
- `cd src-tauri && cargo test --lib`：通过，116 项（含 `TimeoutStream` 读/写超时、`split_uids` 分片、`stored_uids` 续传与 `upsert_message` pending 守卫单测）。
- `pnpm tsc --noEmit`：通过。
- `pnpm test`：通过，29 个测试文件、80 项。
