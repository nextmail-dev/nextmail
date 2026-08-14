# IMAP 流式批取与正文引用资源

状态：已验收

## 目标

降低大型邮箱邮件头同步的 IMAP 命令数量和连接重置概率，同时保留逐封落库、逐封可见与失败后按已落库 UID 续传。同步期间为已读、星标、移动等持久化待办以及按需正文/附件请求保留独立交互容量。

正文选择性获取根据 HTML 的实际 CID 引用下载对应 MIME part，不再仅凭服务端声明的 `image/*` 类型决定是否获取；只有通过既有图片类型、魔数和大小预算校验并成功内联的 part 才从附件列表移除。

## 范围

- 邮件头 `UID FETCH` 每条命令最多包含 20 个 UID，直接消费 IMAP 响应流；每收到一封就解析、原子落库并发布既有最小事件。
- 单账户主动 IMAP 会话预算调整为 3：完整同步固定使用 2 条，保留 1 条给交互路径。
- 账户同步循环与持久化待办循环分离；完整同步期间已读、星标、移动、复制、归档、删除及 Sent/Drafts APPEND 可使用保留的交互连接。
- 按需正文和附件继续由 Tauri Command 并发执行，并与持久化待办共享单账户保留容量。
- 正文 HTML 实际引用且声明为 `image/*` 或可能误标为 `application/octet-stream` 的 CID part 作为候选选择性获取；下载后继续由现有安全图片校验决定是否内联和从附件投影移除。
- 新增 `0028` 窄迁移，只让带 octet-stream Content-ID 候选的既有 HTML 正文缓存重新按新策略获取，不清空普通正文缓存。
- 前端直接使用每条 `sync-progress` 事件的完整载荷逐封更新缓存，并按 revision 拒绝迟到事件，不再为每封邮件触发进度查询 IPC。
- 所有 SQLite 写事务从入口使用 `BEGIN IMMEDIATE`，同步落库与正文、附件、待办等交互写入竞争时等待现有写者，而不是在事务中途升级写锁并终止同步。
- `BODYSTRUCTURE` 附件参数复用完整 MIME 解析器处理 RFC 2047 与 RFC 2231 扩展、分段文件名；新增 `0029` 窄迁移，让已保存为 encoded-word 的异常附件重新获取元数据。
- 更新 ADR 0014 与 `docs/project.md` 的长期同步事实。

## 非目标

- 不新增长期 IMAP Session 池、跨操作 Session 复用、IDLE 或新的秒级同步重试状态机。
- 不改变远程图片、HTML sandbox、CID/data 图片大小预算和文件魔数校验。
- 不把普通未引用附件并入正文下载，也不实现附件分块或断点续传。
- 不改前端 DTO、Query key、事件载荷或界面文案。

## 验证门禁

- Rust 测试固定邮件头批次上限为 20，并覆盖 45 个 UID 拆为 20/20/5。
- Rust 测试固定单账户预算为 3、同步连接为 2，并验证同步占用时第三条交互租约可取得、第四条等待。
- MIME 结构测试覆盖被 HTML CID 引用但声明为 `application/octet-stream` 的图片 part 会进入正文候选，同时仍保留附件元数据直到安全内联成功。
- 迁移测试覆盖只失效可能受旧选择规则影响的 HTML 正文缓存，并把 schema metadata 更新到 28。
- 并发存储测试覆盖第二个写事务等待首个写者释放后成功取得写锁。
- MIME 结构测试覆盖用户样本的 RFC 2231 分段文件名和服务端 RFC 2047 兼容形式。
- 迁移测试覆盖只失效 encoded-word 异常附件对应的正文缓存，并把 schema metadata 更新到 29。
- `cargo fmt --all -- --check`。
- `cargo test --offline --locked`。
- `cargo clippy --offline --locked --all-targets -- -D warnings`。
- `pnpm test`。
- `pnpm build`。
- `git diff --check`。
- Windows 实机：大型邮箱首次同步逐封出现且可断点续传；同步期间连续标记已读/星标、移动邮件和打开未缓存正文；被正文引用的误标类型图片正常显示且不出现在附件区，未引用或无效图片仍作为附件。

## 实施结果

- 邮件头 worker 现在每条 `UID FETCH` 最多请求 20 个 UID，直接逐项消费 async-imap 响应；每项仍独立解析、原子落库并发布既有 `message-arrived`/进度事件，批次中途失败时已完成项可由现有 UID 差集续传保留。
- 单账户会话常量回调为 3/2。完整同步固定租用两条连接；正文、附件和待办共享剩余一条，不建立长期 Session 池。
- `AccountSupervisor` 使用独立的同步与待办唤醒信号，并并发启动两个账户循环。持久化待办不再排在完整同步之后；启动恢复、停止和重认证仍使用同一 generation/状态边界。
- `BODYSTRUCTURE` 将带 Content-ID 的 `image/*` 与 `application/octet-stream` part 交给 HTML 实际引用筛选；下载内容仍经既有类型、魔数、单项 25 MiB 与总计 100 MiB 预算校验。只有成功写成安全 data URL 的 CID 才从附件投影删除。
- 新增迁移 `0028_selective_cid_body_refresh.sql`，只失效带 octet-stream Content-ID 候选的既有 HTML 缓存；普通 HTML/纯文本缓存不受影响，schema metadata 更新到 28。
- ADR 0014 与 `docs/project.md` 已同步记录 20 UID 流式批取、3=2+1 会话预算、独立待办循环和选择性 CID 规则。
- 修复大型邮箱持续同步时前端越来越卡并最终 OOM：`sync-progress` 原先每封邮件都使 TanStack Query 失效并发起一次 Tauri 查询；现在逐条事件载荷直接写入缓存，不再产生查询请求，同时通过 revision 防止多 worker 迟到事件使进度倒退。1000 条突发事件回归测试确认进度连续处理、最终值正确且进度查询零失效。
- 本机日志确认打开邮件或写入已读待办时，延迟事务从读升级为写会直接返回 `SQLITE_BUSY`，完整同步随后以 `storage.message_write_failed` 结束；所有写事务现统一从入口取得 SQLite 写者槽，保留 WAL 并发读取和现有 15 秒等待预算，不增加 IMAP 连接。
- `BODYSTRUCTURE` 文件名解析现在把参数交给既有 `mail-parser` 合并、百分号解码及字符集解码；覆盖 `filename*0*`/`filename*1*` 和服务端 encoded-word 形式。新增迁移 `0029_attachment_filename_refresh.sql`，仅失效已保存为 `=?...` 文件名的邮件正文缓存，使既有异常元数据在下次打开时重取；schema metadata 更新到 29。

## 自动验证

- `cargo fmt --all -- --check`：通过。
- `cargo test --offline --locked`：181 项通过。
- `cargo clippy --offline --locked --all-targets -- -D warnings`：通过。
- `pnpm test`：43 个测试文件、162 项测试通过。
- `pnpm build`：通过；仅保留既有大 chunk 提示。
- `git diff --check`：通过。

## 手动验收重点

1. 用尚未完成首次同步的大型邮箱开始收取，确认列表仍逐封出现且同步可持续推进；中途断网后再次同步只补缺失 UID。
2. 同步持续期间连续执行已读、星标和移动，确认本地乐观状态保持，远端网页端能在完整同步结束前看到对应变化。
3. 同步持续期间打开多封未缓存正文和一个未下载附件，确认请求顺序完成，日志没有新增 `too many connections` 或 `ConnectionReset`。
4. 打开正文引用、但 MIME part 声明为 `application/octet-stream` 的 PNG/JPEG/GIF/WebP/BMP 邮件，确认图片显示且该 part 不出现在附件区。
5. 验证未引用 CID、伪造图片魔数或超出大小预算的 part 不会内联，仍保留为附件；远程图片默认阻止与 sandbox 行为不变。
6. 保持大型邮箱同步运行一段时间，确认进度仍持续更新，邮件列表可正常操作，前端内存不再持续增长或 OOM。
7. 同步大量邮件时连续打开未缓存正文并切换已读/星标，确认同步计数继续推进，日志不再出现 `database is locked` 或 `storage.message_write_failed`。
8. 重新打开包含用户样本附件的邮件，确认显示“浙江广电无线信号测试说明.docx”，下载与系统打开使用同一文件名；普通正文缓存不被重新下载。
