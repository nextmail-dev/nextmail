# 同步 body 回填对服务器已消失邮件的健壮性

状态：已验收

## 目标

开启"收取邮件全文"的账户在同步内 body 回填时，若某封邮件在服务器端已消失（被删除、EXPUNGE 或移动到其他文件夹），不再因 `sync.message_not_found` 失败整条同步并使账户离线。

## 根因

`sync.message_not_found` 在 `src-tauri/src/protocols/imap/session.rs` 的 `fetch_remote_messages` / `fetch_remote_message` 中抛出：当 `UID FETCH BODY.PEEK[]` 对某 UID 返回空数据时，即该邮件在服务器上已不存在。

导致账户离线的路径是同步内 body 回填（`fetch_bodies_worker`，`src-tauri/src/protocols/imap.rs`）：

- 回填逐封拉取本地有头无正文邮件的正文，UID 列表来自本地状态 `pending_body_locations`，而非本次刚拉到的远端 UID 集。
- 上次头同步与本次正文回填之间，邮件在服务器端消失 -> `fetch_remote_messages` 返回空 -> `?` 传播 `sync.message_not_found` -> 整个文件夹同步失败 -> 账户同步失败。
- `mail_runtime` 对所有非鉴权错误直接置 `AccountRuntimeState::Offline`（不看 retryable）。
- 关键时序：`reconcile_flags`（通过 `reconcile_mailbox` 修剪本地已消失邮件）在 `fetch_missing_bodies` 之后执行，所以正文回填先尝试拉取已消失邮件，早于本地 stub 被修剪。头同步 `fetch_summary_batch` 已用 `filter_map` 容忍缺失 UID，只有正文回填路径严格报错。

## 范围

- `fetch_bodies_worker` 把"单封邮件正文不可用"当作非致命跳过，其余错误仍照常失败同步。
- `fetch_missing_bodies` 把本地 pending body UID 与本次 `uid_search` 的远端 UID 集取交集，跳过已知消失的 UID，避免无效请求。
- 新增 `is_message_unavailable_error` 分类 helper，覆盖 `sync.message_not_found` 与同类的 `sync.message_body_missing`。
- 新增 helper 的纯函数单元测试。
- 更新 `docs/project.md` §5 同步模型说明。
- 对同步与邮件流程做一次健壮性排查，结论见"健壮性排查发现"。

## 已确认约束

- 这是服务器/本地不一致的正常情况，不是连通性或鉴权失败；单封邮件消失不应到达 `mail_runtime` 的 Offline 状态机。
- 跳过的本地 stub 由紧随其后的 `reconcile_flags` 修剪，无需在回填层额外删除。
- 不改 `fetch_remote_messages` / `fetch_remote_message` 的契约与严格语义；按需单封路径（`fetch_message_session`）保持严格，给用户显示"该邮件已不可用"是正确 UX。

## 实现路径

- `fetch_bodies_worker` 中把对 `fetch_remote_messages(...).await?` 的直接传播改为 `match`：`Ok` 走原 upsert + 进度逻辑；`Err` 且 `is_message_unavailable_error` 为真时记 `tracing::warn!`、推进 `completed` 并发 `SyncNotice::Bodies`、`continue`；其余 `Err` 仍 `return Err`。
- 跳过时仍推进 `completed`，使进度条能到达 total，不因跳过而停滞。
- `fetch_missing_bodies` 新增 `remote_uids: &HashSet<u32>` 参数；`sync_folder` 不再 `into_iter` 消费 `remote_uids`，改用 `iter().copied()` 并把引用传入。`pending_body_uids` 结果与 `remote_uids` 取交集后才分发给 worker，交集过滤掉的 UID 记 `tracing::debug!`。这处理"同步间被删"的常见情况；"同步内竞态"（`uid_search` 后、FETCH 前被删）仍由上面的 catch+skip 兜底。
- `is_message_unavailable_error` 仿 `mail_runtime` 的 `is_authentication_error` 模式，模块级私有，`matches!` 匹配稳定错误码。

## 健壮性排查发现

对同步与邮件相关流程做了一次全面排查，确认以下结论：

- **已修复**：`fetch_bodies_worker` 对单封邮件消失的容错（本计划主体）。
- **已修复**：`fetch_missing_bodies` 交集过滤，减少对已知消失邮件的无效请求。
- **确认健壮，无需改动**：
  - `parse_message_with_state` 全程 `unwrap_or_default`，邮件解析失败不致命，仅 `spawn_blocking` panic 才报 `sync.message_parse_failed`。
  - `fetch_summary_batch` 用 `filter_map` 容忍缺失/畸形 UID，头同步不会因单封缺失失败。
  - `reconcile_mailbox` 正确修剪本地已消失邮件的 `message_locations`，并保留有 pending operation 的邮件。
  - 待办重放：非可重试 `continue`（标记 failed 并回滚本地乐观更新），可重试 `break`（指数退避，最多 8 次后转 failed），单条失败不阻塞队列。
  - `TimeoutStream` 对 IMAP 读写有逐次超时，stall 连接会失败而非永久卡在 Syncing。
  - 同步失败置 Offline 是设计意图：supervisor 循环在下一个同步间隔自动重试；`Retrying` 状态用于外部命令报错的短重试。不是 bug。
- **潜在改进（未实施，待决策）**：
  - 单个文件夹同步失败（如 `sync.mailbox_open_failed`）经 `sync_session` 的 `?` 中断所有剩余文件夹，整条账户同步失败。对"某个文件夹持续异常"的场景会卡住其他文件夹的同步。隔离文件夹失败是更大的架构改动（需决定跳过策略与上报方式），本轮不做。
  - 按需单封获取（`fetch_message_session`）返回 `sync.message_not_found` 时，本地 stub 留待下次完整同步的 `reconcile_mailbox` 修剪。可考虑在按需路径主动标记/修剪，但属 UX 优化，自愈已存在。

## 非目标

- 不改 `mail_runtime` 的 Offline/Retrying 状态机。
- 不隔离文件夹级同步失败（`sync_session` 仍对单文件夹失败 `?` 中断整条同步）；属更大架构改动，待单独决策。
- 不为按需单封路径增加主动修剪本地 stub（保持现状，下次同步自愈）。
- 不引入 mock IMAP transport 测试基建；`fetch_bodies_worker` / `fetch_missing_bodies` 依赖真实 `async_imap::Session<T>`，控制流靠代码审查与既有测试套件不回归保证。
- 不改头同步、`reconcile_flags`、UID validity 处理。

## 验证门禁

- 新增 helper 单元测试随 `cargo test` 通过。
- `cargo fmt --all -- --check`、`cargo test --offline --locked`、`cargo clippy --offline --locked --all-targets -- -D warnings`、`git diff --check` 通过。

## 验证结果

- 2026-08-12：自动验证通过。
- 2026-08-13：用户手动验收通过。
- `cargo fmt --all -- --check` 通过。
- `cargo test --lib` 通过，共 170 项测试（含新增 `classifies_message_unavailable_errors`）。
- `cargo clippy --locked --all-targets -- -D warnings` 通过，无警告。
- `git diff --check` 通过。
- 说明：`--offline` 因本机 registry 索引缺 `tauri-plugin-updater` 无法解析，本轮以非离线模式运行 `--locked`；行为等价，仅依赖解析走网络镜像。
- 附带修复：发现 `protocols::compose::tests::builds_unicode_multipart_message_without_bcc_header` 在 v0.4.0 下断言过时（硬编码 `x-mailer: nextmail/0.3.1`，而 `compose.rs` 用 `env!("CARGO_PKG_VERSION")` 生成 `0.4.0`）。改为用 `env!("CARGO_PKG_VERSION")` 动态断言，避免后续版本升级再漏改。此为预存发布遗留，与本计划主体无关。
