# NextMail 改进意见（Codex 实施清单）

本文档汇总对 NextMail（Tauri 2 + React 19 + TypeScript + Rust）项目结构与代码的优化建议，供 Codex 逐条实施。每条包含：**位置**（file:line）、**问题**、**实施**、**验收**。

## 使用说明

- **优先级**：P0 = 正确性 bug / 热路径性能；P1 = 架构与可维护性；P2 = 性能与重渲染；P3 = 清理与工具链。
- **分批原则**：每条尽量独立可提交。涉及同一文件的多条（如 `imap.rs`、`repository.rs`、`draft_repository.rs`）建议合并到同一批次，避免冲突。
- **不要破坏现有测试**：每批改完跑对应验证命令；重构类改动应先补/调测试再改实现。
- **保持架构边界**：参见 `docs/architecture.md`。协议库类型不得越过 Adapter；命令错误只返回稳定错误码；核心层不得依赖 Tauri/SQLx/协议库。

### 验证命令

```powershell
# 前端
pnpm test
pnpm build

# Rust（在项目根目录）
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

### 建议落地顺序（跨区域）

1. P0 全部（正确性 + 热路径）
2. P1 中的分层修复（`draft_repository` 业务逻辑上移、`MailRepository` 拆分、端口注入）
3. P1 中的前端结构（`MainShell` 拆分、偏好双数据源合并）
4. P2 / P3 清理与工具链

---

## 一、前端

### F1 【P0】邮件详情 / 附件状态 query key 错位导致不刷新

- **位置**：`src/features/mail/MessageViewer.tsx:54`（key 定义）、`src/features/mail/MainShell.tsx:108`、`src/features/mail/MessageViewer.tsx:60`
- **问题**：详情查询 key 为 `["message", accountId, mailboxId, messageId]`，但两处失效调用把 `messageId` 放到了 `mailboxId` 槽位，React Query 前缀匹配永远命中不了：
  - `MainShell.tsx:108` 的 `message-content-changed` 监听失效 `["message", selectedAccountId, event.payload.messageId]` → 后台下载完正文后阅读器不自动刷新。
  - `MessageViewer.tsx:60` 的 `attachmentMutation.onSuccess` 失效 `["message", accountId, messageId]` → 附件 availability 徽标卡在 "queued"。
  - 同文件正确的 2 段前缀写法见 `MainShell.tsx:119`、`MessageViewer.tsx:80`、`MessageListPane.tsx:63`。
- **实施**：两处失效都改为 2 段前缀 `["message", accountId]`（`MainShell` 用 `selectedAccountId`）。事件 payload 不含 mailboxId，前缀匹配是正确做法。
- **验收**：新增 `MessageViewer` 测试，mock `useQueryClient`，触发附件下载成功后断言 `invalidateQueries` 被以 `["message", accountId]` 调用。手动验证：下载附件后徽标从 queued 变 available；后台拉取正文后阅读器自动更新。

### F2 【P1】外观偏好 Zustand + React Query 双数据源

- **位置**：`src/app/appearance.ts`（Zustand store）、`src/app/App.tsx:103-116,157-196`、`src/features/preferences/SettingsApp.tsx:69-88`、`src/features/composer/ComposerApp.tsx:40`
- **问题**：外观偏好同时存在 Zustand store 和 `["preferences"]` query cache，靠 3 处 `useEffect` + 1 个事件桥手动同步；`applyAppearance` 对同一份数据被调 2–3 次；乐观更新回滚只回滚 Zustand。Zustand store 未提供 React Query cache + `onMutate` 做不到的能力。
- **实施**：删除 Zustand store。读用 `useQuery(["preferences"])`；写用 `useMutation`，`onMutate` 中 `cancelQueries` 后 `setQueryData` 写入新值，`onError` 回滚到 `previous`。移除所有手动同步 effect（App.tsx:177、SettingsApp.tsx:69、ComposerApp.tsx:40）。事件桥 `AppearanceEventBridge` 保留，但只做 `setQueryData`。
- **验收**：`pnpm test` 通过；手动验证设置窗口改主题/语言后主窗口与写信窗口同步、且失败时回滚。确认无 `useAppearanceStore` 残留引用。

### F3 【P1】ComposerApp 关闭处理每次按键都重新订阅

- **位置**：`src/features/composer/ComposerApp.tsx:138-164`（effect 依赖 `saveNow`），`saveNow` 定义在 `:121`
- **问题**：`onCloseRequested` effect 依赖 `saveNow`，而 `saveNow` 依赖 `[to, cc, subject, content, bcc, dirty, ...]`，每次按键重建 listener；unlisten 是异步的，快速编辑可能堆叠多个 listener 或出现空窗。
- **实施**：把 `saveNow` 存进 ref（每次渲染 `saveNowRef.current = saveNow`），effect 依赖只留 `[draft.id, sender.id]`，handler 内读 `saveNowRef.current`。
- **验收**：编辑正文时用 devtools 确认 Tauri `onCloseRequested` listener 不再反复增删；关闭窗口仍能触发保存。

### F4 【P1】MainShell 是 273 行上帝组件

- **位置**：`src/features/mail/MainShell.tsx`（10 个 `useState` 于 :28-37，8 个 `useEffect` 于 :67-158，5 个 Tauri 监听，resize 钳制数学 :138-151）
- **问题**：状态、事件监听、布局计算全堆在一个组件；resize effect 读 `folderPaneWidth`/`messagePaneWidth` 却未列入依赖（lint 违规，手动 resize 后用到 stale closure）。
- **实施**：抽三个 hook：`useMailboxSelection`（account/mailbox/message 选择 + 相关 effect）、`useTauriEventListeners`（:95-124 的 5 个监听）、`usePaneLayout`（resize 状态 + 钳制逻辑，用 ref 或函数式更新消除 stale closure 与 lint 违规）。
- **验收**：`pnpm test` 通过；新增 `usePaneLayout` / `useTauriEventListeners` 单测；手动验证拖动分栏、窗口缩放钳制、事件失效均正常。

### F5 【P2】邮件列表未 memo + 内联 handler 导致搜索时全量重渲染

- **位置**：`src/features/mail/MessageListPane.tsx:89-103`（`MessageRow` 未 memo，`onClick`/`onToggleFlag` 内联），过滤 `:48-54`
- **问题**：搜索框每次输入重渲染全部可见行；过滤数组每次按键重新分配。
- **实施**：`MessageRow` 包 `React.memo`；父组件用 `useCallback` 稳定 `onSelect`/`onToggleFlag`（只依赖 `accountId`/`mailboxId`/`selectedMessageId`）；过滤结果 `useMemo([allItems, normalizedSearch])`。
- **验收**：React DevTools Profiler 确认搜索输入时 `MessageRow` 不再全部重渲染；`pnpm test` 通过。

### F6 【P2】`flattenMailboxHierarchy` 未 memo

- **位置**：`src/features/mail/MailboxPane.tsx:73-75`
- **问题**：每次渲染都建树、排序、扁平化；`mailboxes` 在同步期每 1.5s 是新数组引用。
- **实施**：`useMemo(() => flattenMailboxHierarchy(mailboxes), [mailboxes])`，可见项再 `useMemo([mailboxItems, collapsedFolderIds])`。
- **验收**：`MailboxPane.test.tsx` 通过；折叠/展开时树不重建。

### F7 【P2】轮询过密（已有事件驱动失效）

- **位置**：`src/features/mail/MainShell.tsx:49-60`（`draftsQuery` 3s 于 :53，`pendingOperationsQuery` 5s 于 :59，`progressQuery` 1.5s 于 :47 合理保留）
- **问题**：草稿、待办都已有 `send-job-changed`/`pending-operation-changed` 事件监听做失效，3s/5s 轮询是过紧的冗余安全网。
- **实施**：草稿/待办轮询放宽到 15–30s，或移除、改为窗口聚焦重取（`refetchOnWindowFocus`）。`progressQuery` 保持不变。
- **验收**：手动验证：写信窗口保存草稿后主窗口草稿列表更新；待办状态变化后列表更新；空闲期不再每 3s/5s 打 invoke。

### F8 【P3】`src/App.tsx` 是多余的再导出 shim

- **位置**：`src/App.tsx`（单行 `export { App as default } from "./app/App"`）、`src/main.tsx:6`
- **实施**：`main.tsx` 改为 `import App from "./app/App";`，删除 `src/App.tsx`。
- **验收**：`pnpm build` 通过。

### F9 【P3】`formatBytes` 跨组件重复

- **位置**：`src/features/composer/ComposerApp.tsx:359-363`、`src/features/mail/MessageViewer.tsx:300-304`（`formatAddresses` 亦相近）
- **实施**：新建 `src/lib/format.ts`，移入 `formatBytes`（及 `formatAddresses`），两处改 import。
- **验收**：`pnpm test` 通过；无重复实现。

### F10 【P3】`composer.css` 深色模式下高亮文字不可见

- **位置**：`src/styles/composer.css:36-38`
- **问题**：`span[style*="background-color"] { color: #202124 }` 强制近黑文字，深色模式深色高亮背景上不可见。
- **实施**：限定到浅色作用域：`:root[data-theme="light"] .nextmail-editor-content span[style*="background-color"] { color: #202124 }`（含 `system` 浅色变体），或改用随主题翻转的 CSS 变量。
- **验收**：浅色/深色模式下高亮文字均可见。

### F11 【P3】`index.html` 硬编码 `lang="zh-CN"`

- **位置**：`index.html:2`
- **问题**：en-US 用户首帧（含启动壳）lang 属性错，影响屏幕阅读器与字体渲染，直到 `applyAppearance` 运行。
- **实施**：改为 `lang="en"`（或用 `navigator.language` 内联脚本初始化），交由 `applyAppearance` 纠正。
- **验收**：英文环境下首帧 `documentElement.lang` 正确。

### F12 【P3】前端测试覆盖缺口

- **位置**：`MessageViewer.tsx`、`MainShell.tsx`、`MessageListPane.tsx`、`ComposerApp.tsx`、`app/App.tsx` 均无测试
- **实施**：优先补两条：(1) `MessageViewer` mutation 失效 key 断言（配合 F1）；(2) `MainShell` 事件监听 wiring 测试。`ComposerApp` 自动保存状态机次之。
- **验收**：新增测试通过；`pnpm test` 覆盖率提升。

---

## 二、Rust 后端

### R1 【P0】`upsert_message` 多表写入无事务 + 附件逐条 INSERT

- **位置**：`src-tauri/src/storage/repository.rs:473-603`（附件循环 :583-600）
- **问题**：`upsert_message` 顺序执行 SELECT 存在性 → SELECT 去重 → INSERT messages → INSERT message_locations → INSERT message_bodies → 循环 INSERT attachments，全程直接打 `&self.pool`，无 `begin()`；中途失败留半成品（如邮件已插入但附件缺失）。附件逐条 INSERT 也是 N 次往返。
- **实施**：
  1. 用 `let mut tx = self.pool.begin().await.map_err(...)?;` 包住整段写，所有 `&self.pool` 改为 `&mut *tx`，末尾 `tx.commit().await.map_err(...)?;`。`content.write_raw` 若也写库需在同一事务或显式说明为何独立。
  2. 附件改多行 `INSERT INTO attachments VALUES (…),(…),… ON CONFLICT(message_id, part_index) DO UPDATE SET …`，从 `&message.attachments` 构造。
- **验收**：新增单测：构造多附件邮件，模拟中间语句失败（或断言事务回滚后无残留）；`cargo test --workspace` 通过；`cargo clippy` 无 warning。

### R2 【P0】每次 TLS 连接都重新加载系统证书

- **位置**：`src-tauri/src/protocols/imap.rs:1184-1205`（`connect_tls`）、`src-tauri/src/adapters/mail_connection.rs:209-234`
- **问题**：`connect_tls` 每次调用 `rustls_native_certs::load_native_certs()` 重建 `RootCertStore` + `ClientConfig`。`AsyncImapProvider` 每操作开新连接，`drain_pending_operations` 每待办一条连接 → 10 待办 + 1 同步 = 11 次证书加载；Windows 上每次枚举 schannel 证书库。
- **实施**：构建一次 `TlsConnector`（`OnceCell<Arc<TlsConnector>>` 或存到 `AsyncImapProvider` / 测试可注入），每次连接 clone。
- **验收**：单测或日志确认同一进程多次连接只加载一次根证书；同步流程功能不变。

### R3 【P0】正文按需拉取是网络层 N+1

- **位置**：`src-tauri/src/protocols/imap.rs:810-851`（摘要循环内逐封调 `fetch_raw`），`fetch_raw` :914
- **问题**：头已按 100 批拉取，但每个命中 `should_download_body` 的邮件单独 `UID FETCH <uid> BODY.PEEK[]`，一封一次往返；几百封新邮件 = 几百次串行往返。
- **实施**：摘要遍历时收集需要正文的 UID，再按 `FETCH_BATCH_SIZE` 分批 `UID FETCH <uid-set> BODY.PEEK[]`（async-imap 返回 fetch 流），按 UID 归并后落库。
- **验收**：`cargo test --workspace` 通过；大邮箱首同步往返数显著下降（可加临时日志统计 FETCH 次数）。

### R4 【P0】`reconcile_mailbox` 是 SQL 层 N+1

- **位置**：`src-tauri/src/storage/repository.rs:690-713`
- **问题**：对每条本地 `message_locations` 行单独 `SELECT COUNT(*) FROM pending_operations` 再单独 `DELETE`；reconcile 每文件夹每次同步都跑，查询数随本地邮箱大小增长。
- **实施**：改成集合式单条 `DELETE FROM message_locations WHERE id IN (SELECT l.id FROM message_locations l LEFT JOIN pending_operations o ON … WHERE l.mailbox_id = ? AND l.uid_validity = ? AND l.uid NOT IN (...) AND o.id IS NULL)`。
- **验收**：`cargo test --workspace` 通过；reconcile 行为不变（既有 `operation_repository` 的 reconcile 相关测试应仍过）。

### R5 【P1】`draft_repository` 混入用例 + 表现层逻辑（分层泄漏）

- **位置**：`src-tauri/src/storage/draft_repository.rs`：`create_message_action_draft` :104-256（接收 i18n label 参数拼引用块）、`reply_recipients` :866、`unique_addresses` :879、`prefixed_subject` :915、`format_addresses` :898、`editor_document_from_text` :959（在 Rust 拼 Tiptap JSON 文档）、`escape_html` :986
- **问题**：回复/转发草稿生成（收件人规则、Re:/Fwd: 主题、引用块、Tiptap 文档、HTML 转义）是**用例/表现层**逻辑，却嵌在存储 repository 里。架构文档规定回复/转发是 Rust 职责，但应落在 application 用例层，repository 只持久化。这也让这些规则无法脱离真实 SQLite 单测。
- **实施**：
  1. 在 `application` 层新增 `compose_reply_forward_draft(...)`（纯计算：从原邮件字段产出 `DraftContent`，含收件人、主题、正文、Tiptap 文档、引用块），调用上述 helper。
  2. `draft_repository` 的 `create_message_action_draft` 拆成：repository 只负责读原邮件字段 + 写新草稿；用例层负责计算。
  3. `escape_html` 移到 `protocols/html.rs`（或既有清洗模块）；`editor_document_from_text`、`format_addresses`、`reply_recipients`、`unique_addresses`、`prefixed_subject` 上移到 application/domain。
- **验收**：新增纯单测覆盖 reply-all 收件人去重、Re:/Fwd: 主题、Tiptap 文档生成（无需数据库）；`draft_repository` 不再出现 Tiptap JSON 构造与 HTML 转义；`cargo test --workspace` 通过；`storage` 模块不 import 表现层逻辑。

### R6 【P1】`MailRepository` 是横跨 3 文件的上帝类型

- **位置**：`src-tauri/src/storage/repository.rs:40`、`draft_repository.rs:57`、`operation_repository.rs:39`（均为 `impl MailRepository`，合计约 55 个方法）
- **问题**：当前 3 文件拆分只是文件级，`MailRepository` 仍是单一类型，加草稿方法会出现在同步逻辑旁；难以独立单测。
- **实施**：按聚合根拆成共享 `SqlitePool` 的子结构（建议命名）：
  - `MailReadRepository`（repository.rs 读取类：list_mailboxes / list_messages / get_message_detail / raw_message / attachment_context）
  - `SyncSinkRepository`（repository.rs 写入类：upsert_mailbox / upsert_message / complete_mailbox / reconcile_mailbox / pending_body_locations）
  - `DraftRepository`（draft_repository.rs 前半：create/save/附件/threading）
  - `SendJobRepository`（draft_repository.rs 后半：queue/claim/complete/fail/retry/send_mime）
  - `OperationRepository`（operation_repository.rs：queue_* / claim / complete / rollback / retry）
  - `MailboxRoleRepository`（见 R7）
  跨结构事务（如 `upsert_message`）通过传入 `&Pool` 或 `&mut Transaction<'_, Sqlite>` 共享。`MailRepository` 可保留为持有子仓库的 facade，或直接让 `state.rs` 装配各子仓库。
- **验收**：`cargo test --workspace` 通过；`cargo clippy` 无 warning；各子仓库可独立单测；`commands/mod.rs` 与 runtime 调用点相应更新。

### R7 【P1】`mailbox_for_role` 角色映射放错文件

- **位置**：`src-tauri/src/storage/operation_repository.rs:331-426`（`mailbox_for_role` / `mailbox_role_for_id` / `set_mailbox_role_mapping`）
- **问题**：文件夹角色配置与 pending operations 无关，只是凑在同一 `impl MailRepository` 里。
- **实施**：随 R6 移入 `MailboxRoleRepository`（或主 repository），从 operation_repository 移出。
- **验收**：`cargo test --workspace` 通过；operation_repository 只剩待办操作逻辑。

### R8 【P1】六边形边界泄漏：application 层依赖具体 adapter，runtime 硬编码 provider/repo

- **位置**：`src-tauri/src/application/service.rs:10-23,42-49`（直接 import 并 `new()` `AccountsStore`/`BootstrapStore`/`PreferencesStore`/`ReadingPreferencesStore`，仅 `CredentialStore`/`ConnectionTester` 走 `Arc<dyn>`）；`src-tauri/src/mail_runtime.rs:44,551-558`、`src-tauri/src/composer_runtime.rs:30,607-615`（硬编码 `AsyncImapProvider` 与 `MailRepository::open`）
- **问题**：application 层耦合文件存储实现，无法脱离真实文件系统单测；`ImapSyncProvider` trait 存在却未用于注入，supervisor/drain/重试路径无法用 mock 测。
- **实施**：
  1. `core/ports.rs` 补 `AccountStore`/`BootstrapStore`/`PreferencesStore`/`ReadingPreferencesStore` 等 port trait，`AppService::new` 全部 `Arc<dyn …>` 注入。
  2. `MailRuntime::new` 接受 `Arc<dyn ImapSyncProvider>`；引入 repository port（覆盖 runtime 用到的读/命令方法）或至少注入 repository 句柄。
  3. `ComposerRuntime` 复用 `MailRuntime` 已初始化的 repository，不再 `MailRepository::open`（也消除 R11 重复）。
- **验收**：`cargo test --workspace` 通过；可写 mock provider/repo 单测覆盖 supervisor 启动、drain、重试路径。

### R9 【P1】IMAP 连接建立样板重复 6 次

- **位置**：`src-tauri/src/protocols/imap.rs:37-258`（`ImapSyncProvider` 6 个 trait 方法各含 ~35 行 `match account.security { None/Tls/StartTls }` 连接+登录块）
- **问题**：约 210 行复制粘贴；连接路径任何改动（超时、证书缓存）都要改 6 处，易漂移。
- **实施**：抽 `async fn connect_session<T>(account) -> CommandResult<Session<T>>`（含 TCP/TLS/greeting/login），各 trait 方法调用后分派到对应 `*_session`。
- **验收**：`cargo test --workspace` 通过；连接路径仅一处实现；配合 R2 的证书缓存只改一处。

### R10 【P1】`sync_session` 是 ~225 行上帝函数

- **位置**：`src-tauri/src/protocols/imap.rs:687-912`
- **问题**：列文件夹、capability、select/examine、UNSEEN 搜索、邮箱 upsert、ALL 搜索+过滤+排序、批量头拉取、逐封正文拉取、解析、upsert、正文回填、`1:*` flag 拉取、reconcile、complete、notify 全在一处；错误处理是一整面 `map_err(|_| …)` 墙。
- **实施**：抽 `sync_folder(...)`（单文件夹完整流程），内再分 `fetch_summaries` / `backfill_bodies` / `reconcile_flags`；外层只剩文件夹循环 + 最终 notify。
- **验收**：`cargo test --workspace` 通过；`sync_session` 主体降至可一屏阅读。

### R11 【P1】`imap.rs` 混杂 4 个不相干职责

- **位置**：`src-tauri/src/protocols/imap.rs`（1363 行）
- **问题**：IMAP 编排、MIME 解析映射、文件夹名编码、同步策略数学挤在一处；后三者不碰 session 却无法独立测试。
- **实施**：拆成目录模块：
  ```
  protocols/imap/
    mod.rs       // provider + trait impl + connect/login/read_greeting/connect_tls + sync_session
    session.rs   // apply_operation_session / append/replace/wait/fetch *_session
    parse.rs     // parse_message* / attachment_summaries / addresses / message_flag_state
    encoding.rs  // decode_modified_utf7* / mailbox_role
    policy.rs    // should_download_body / sync_policy_cutoff（或上移 domain）
  ```
- **验收**：`cargo test --workspace` 通过；`parse.rs`/`policy.rs` 可脱离网络单测；`mod.rs` 显著缩短。

### R12 【P1】100–200 行方法需 extract-method

- **位置**：`create_message_action_draft` `draft_repository.rs:104`（~152，随 R5 处理）、`get_message_detail` `repository.rs:153`（~113）、`import_message_as_draft` `draft_repository.rs:281`（~99）、`queue_flag_operations` `operation_repository.rs:74`（~90）、`claim_pending_operation` `operation_repository.rs:439`（~85）
- **实施**：按职责抽小函数（行映射、事务包装、按 kind 分派）。`get_message_detail` 把行映射抽 `FromRow` 结构（见 R14）；`claim_pending_operation` 把状态机与投影更新分开。
- **验收**：`cargo test --workspace` 通过；各方法主体可一屏阅读。

### R13 【P1】`drain_pending_operations` 元组匹配控制流难读

- **位置**：`src-tauri/src/mail_runtime.rs:560-647`
- **问题**：`AppendSent`/`AppendDraft` 用 `(Ok(dest), Ok(hash)) => … / (Err(e), _) | (_, Err(e)) => Err(e)` 元组匹配，两分支重复 `read_send_mime` + provider 调用逻辑；retry 路径 bug 会静默丢操作。
- **实施**：抽 `run_append_sent` / `run_append_draft`，用正常 `?`；主循环只按 `work.kind` 分派。
- **验收**：`cargo test --workspace` 通过；行为不变。

### R14 【P3】用 `sqlx::FromRow` 取代手写行映射

- **位置**：`draft_repository.rs:130-141`（`create_message_action_draft` 内 `row.try_get`）、`repository.rs:798`（`message_list_item_from_row`）等
- **问题**：手写逐列 `row.try_get("col")` 冗长易错。
- **实施**：为行结构 `#[derive(sqlx::FromRow)]`，用 `query_as`。随 R6 子结构化时一并改造。
- **验收**：`cargo test --workspace` 通过；映射代码显著减少。

### R15 【P3】`map_err(|_| CommandError::new(...))` 丢弃原始错误

- **位置**：`repository.rs:473-603`（`upsert_message` 内 6 处）等 storage 全域
- **问题**：存储失败时丢失 `sqlx::Error` 细节，排查困难。
- **实施**：抽 `fn map_db(err: sqlx::Error, code: &str) -> CommandError`，内部 `tracing::error!(?err, code)` 记录原错误再返回稳定码（命令错误仍只返回稳定码，符合架构）。或 `#[cfg(debug_assertions)]` 下记录。
- **验收**：`cargo test --workspace` 通过；调试构建下存储失败日志含原始错误；命令返回值不变（仍是稳定码）。
- **备注**：需引入 `tracing` 依赖（若尚未引入），并在 `lib.rs` 初始化 subscriber。

### R16 【P3】集中 storage 映射 helper

- **位置**：`role_to_db`/`role_from_db`（`repository.rs:842-864` 与 `operation_repository.rs:831-853` 重复且签名漂移）、`storage_read_error`/`read_error`（三处）、`decode_addresses`（`repository.rs:833` 与 `draft_repository.rs:862`）、`now`/`unix_timestamp`（`repository.rs:893`、`imap.rs:1177`、`composer_runtime.rs:663`、`service.rs:368`）
- **实施**：新建 `storage/mapping.rs` 放 DB↔domain 转换（role/policy/status/availability/addresses）；`now`/`unix_timestamp` 放 `core/util.rs`。统一签名。
- **验收**：`cargo test --workspace` 通过；三处 repo 文件各瘦身且无重复。

### R17 【P3】冗余再导出 + 导入路径不一致

- **位置**：`src-tauri/src/error.rs`（单行 `pub use crate::core::{CommandError, CommandResult}`）、`src-tauri/src/domain/mod.rs`（单行 `pub use crate::core::*`）
- **问题**：同一 `CommandError`/domain 类型有 `crate::error`/`crate::core`/`crate::domain` 三条路径，各模块用法不一致，混淆分层。
- **实施**：统一到 `crate::core`，删除两个再导出文件（或保留 `domain` 但实际拆分 domain 类型），更新所有 import。
- **验收**：`cargo test --workspace` + `cargo clippy` 通过；grep 确认无 `crate::error`/`crate::domain` 残留（或仅剩有意保留的一条）。

### R18 【P3】共享测试夹具

- **位置**：`operation_repository.rs:866` 的 `seeded_repository()` 是好榜样，但 `draft_repository`/`repository` 各自重搭
- **实施**：建 `storage/test_support.rs`（`#[cfg(test)]`），共享 `seeded_repository` + 种子账户/邮件/文件夹。三个 repo 测试套件复用。
- **验收**：`cargo test --workspace` 通过；测试夹具不再重复。

### R19 【P3】`initialize_data_directory` 用了阻塞 `std::fs`

- **位置**：`src-tauri/src/application/service.rs:109-115`
- **问题**：`async fn` 内用 `std::fs::create_dir` 阻塞调度线程（仅初始化时，影响低）。
- **实施**：改 `tokio::fs::create_dir`（及同段其他 `std::fs` 调用）。
- **验收**：`cargo test --workspace` 通过；初始化流程不变。

---

## 三、工具链

### T1 【P3】无 ESLint / Prettier / CI

- **位置**：项目根（无 `.eslintrc*`/`eslint.config.*`/`.prettierrc*`/`.github/workflows`）
- **问题**：Rust 有 `cargo clippy -D warnings` 强制，前端只靠 `tsc` + vitest，无自动化 lint/格式/CI 把关；query key 类 bug 本可被 lint 规则或测试拦下。
- **实施**：
  1. 加 `eslint` + `typescript-eslint`（启用 `react-hooks` 规则，可拦 F3/F4 类依赖问题）+ `prettier`；`package.json` 加 `lint`/`format` 脚本。
  2. 加 GitHub Actions：`pnpm install` → `pnpm lint` → `pnpm test` → `pnpm build` → `cargo clippy --workspace --all-targets -- -D warnings` → `cargo test --workspace`。
- **验收**：`pnpm lint` 通过；CI 在 PR 上跑全套检查。

### T2 【P3】`components.json` 引用不存在的 `@/hooks` 别名

- **位置**：`components.json`（`aliases.hooks` 指向 `@/hooks`，但无 `src/hooks` 目录）
- **实施**：要么建 `src/hooks`（放 R12/F4 抽出的 hook），要么从 `components.json` 移除该别名。建议前者（配合 F4）。
- **验收**：shadcn add 命令可正常工作；别名一致。

---

## 附：本轮不改动（已达标）

- 命令层 55 个 command 一致地薄包装委托给 service/runtime。
- i18n 中英文 key 完全对称（各 338，零缺失）。
- `app/types.ts` 规范使用 discriminated union 与字符串字面量类型，全程零 `any`。
- `api.ts` 薄且全类型化，统一 `invoke<T>`。
- 迁移文件 0001–0007 增量干净。
- CSP、rustls 单一 crypto provider、凭据走系统 keyring 等安全基线扎实。
- CSS 按职责分文件（theme/base/globals/composer）组织清晰。
