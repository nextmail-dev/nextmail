# NextMail 新会话交接

更新时间：2026-08-01

本文是跨会话接手入口，不替代技术参考、架构、ADR、iteration 或 change。接手者必须以当前检出代码、最近 change 和仍有效 ADR 为事实来源；若本文记录的提交或工作区快照已经变化，以新会话实际检查结果为准，不要 reset 或覆盖用户修改。

## 1. 当前快照

- 产品版本：`0.1.0`。
- 文档整理时本地分支：`main`，代码基线 `db4b806 feat: complete pre-release bug fixes`。
- 文档整理时 `origin/main`：`92b4197 fix: submit mail search explicitly`；本地代码基线领先远端一项提交。新会话必须重新执行 `git status --short` 和 `git log -3 --oneline --decorate`，不得根据这条快照盲目推送、回退或清理。
- 第二十一阶段已经于 2026-08-01 通过手动验收。
- 第二十二阶段已经于 2026-08-01 验收：重写中英文 GitHub README，并新增只在 `v*` tag push 时构建 Windows x64、macOS Universal 与 Linux x64、成功后发布 Release 的工作流。用户已经明确要求提交、推送并发布 `v0.1.0`。
- 第二十二阶段不改变产品代码、依赖、数据格式或安全策略。当前没有后续活动阶段；新范围仍须先在 `docs/iterations/` 建档确认。

## 2. 新会话必读顺序

1. 本文件。
2. `docs/technical-reference.md`：当前能力、技术栈、Command/Event、数据、同步、编辑器和安全事实。
3. `docs/architecture.md`：稳定分层、窗口、存储、协议、同步和安全边界。
4. `docs/development.md`：环境、验证和分批交付规则。
5. `docs/plans/master-plan.md`：已经完成的阶段和未排期设想。
6. 与任务直接相关的最近 iteration；当前最新为 `docs/iterations/0021-pre-release-bug-fixes.md`。
7. 与任务直接相关的最近 change；当前最新为 `docs/changes/0058-pre-release-bug-fixes.md`，本次文档整理记录在 `0059`。
8. `docs/adr/` 下仍有效 ADR。`0001` 是已被 `0006` 取代的历史记录；当前重点为 `0002` 至 `0015`，其中 `0007` 的同步调度和会话并发分别由 `0013`、`0014` 修订。
9. 当前 `git status`、提交图、相关源码、迁移和测试。文档与代码冲突时先用代码、最近 change 和有效 ADR核验，再修正文档。

## 3. 产品与平台状态

NextMail 是 Tauri 2 + React 19/TypeScript + Rust 的本地优先桌面邮件客户端。Windows 10 22H2+ x64 是主要实机验收平台；macOS 12+ Intel/Apple Silicon 是目标平台。阶段十三的通知案例曾在 Windows 和 macOS 验收，但后续新窗口或新行为未实际执行时不得笼统宣称 macOS 通过。Linux 不做深度适配。

已实现的主能力：

- 首次启动、用户选择可迁移数据目录、账户自动发现和真实 IMAP/SMTP 密码认证。
- 多账户添加、编辑、重新认证、切换、安全移除、角色映射和独立同步策略。
- SQLite 离线邮件视图、头部优先同步、按需正文、账户可选全文同步、逐封落库和持久化离线操作。
- IMAP 文件夹在线创建、重命名、层级移动、删除、全部已读，以及与服务器顺序分离的本地同层排序。
- 安全 HTML/CSS 阅读、远程图片控制、标准 CID、受限 data 图片、误标 octet-stream 图片和 BMP、附件、原始 EML。
- 当前账户/当前文件夹范围的 SQLite FTS5 搜索；按 Enter 或搜索按钮显式提交。
- Tiptap/ProseMirror + CodeMirror 富文本编辑、HTML 源码/预览、模板、默认签名、变量、回复/回复全部/转发、内嵌图片。
- 用户关闭 Composer 时显式决定是否保存草稿；持久化 SMTP 发件、Sent/Drafts APPEND 和 Drafts 定向刷新。
- Windows/macOS 窗口壳、窗口位置/大小记忆、独立设置/账户/定义/预览/原文窗口、中文/英文、浅/深/系统主题和主题色。
- 全局/账户/文件夹通知偏好与 NextMail 自有桌面通知窗口。

未实现或未排期：

- IMAP IDLE。当前没有无 IDLE 轮询、固定兜底同步或秒级失败重试。
- 会话聚合、跨账户搜索、统一收件箱、托盘和系统通知中心历史/勿扰集成。
- POP3、Google/Microsoft OAuth、跨机重绑定、正式代码签名/公证和自动更新；阶段二十二只增加 tag 驱动的未正式签名预览构建与 GitHub Release，不等于发布硬化完成。
- 联系人、规则、日历、PGP/S-MIME、EML/MBOX 导入导出和 Linux 深度适配。

## 4. 技术栈与仓库结构

前端使用 React 19.1、TypeScript 5.8、Vite 7、TanStack Query 5、react-i18next、Tailwind CSS 4、Radix Primitives、Tiptap/ProseMirror 3.27.3 和 CodeMirror 6。测试为 Vitest、Testing Library 和 jsdom。

Rust 使用 Tauri 2、Tokio、SQLx 0.9/SQLite WAL、async-imap 0.11、lettre 0.11、mail-parser 0.11、mail-builder 0.4、Ammonia 4、cssparser 0.37、rustls 0.23 和 keyring 4.1。

仓库只有 `src-tauri/Cargo.toml` 一个 Cargo package。不得创建根 Cargo Workspace、根 `Cargo.toml`/`Cargo.lock`/`target` 或业务子 crate。Node 依赖全部由 pnpm 管理；当前不使用 Python，未来确需 Python 时统一使用 uv。

主要目录：

```text
src/
  app/                 IPC、DTO、Query key、外观、语言、平台入口
  components/ui/       自有基础组件与滚动区
  components/window/   跨平台标题栏
  features/            accounts/composer/mail/notifications/onboarding/preferences
  locales/             zh-CN、en-US
  styles/              语义主题和全局样式
src-tauri/
  capabilities/        各窗口最小权限
  migrations/          只增不改的 SQLx 迁移，当前到 0024
  src/core/            无 Tauri/SQLx/协议库的 DTO、错误和 ports
  src/application/     账户与纯业务组合用例
  src/adapters/        JSON、Keyring、发现、连接测试和系统集成
  src/protocols/       IMAP/SMTP/MIME/HTML/TLS Adapter
  src/storage/         SQLite 与内容存储的窄 Repository
  src/commands/        薄 Tauri Command
  src/state.rs         唯一组合根
  src/mail_runtime.rs  账户 Supervisor 门面
  src/composer_runtime.rs
  src/notification_runtime.rs
testdata/mail-rendering/ 正式邮件保真与恶意内容回归语料
docs/                 当前参考、架构、计划、iteration、change、ADR
```

## 5. 不可破坏的架构边界

- React 不直接访问 SQLite、邮件服务器、任意文件系统或系统凭据库。
- 所有 IPC 集中通过 `src/app/api.ts` 和稳定 DTO；业务组件不散落裸 `invoke`。
- TanStack Query 管理本地读取模型。事件只携带公开 ID、状态、进度或修订并触发精准失效/重读，不推完整正文。
- `core` 不依赖 Tauri、SQLx、`tracing` 或具体协议库；第三方类型停留在 Adapter/Repository 内。
- `state.rs` 装配具体实现并通过 ports 注入；application/runtime 不重新硬编码 Adapter。
- Command 保持薄并统一返回 `CommandError { code, params, retryable }`。
- 数据访问始终按 `account_slot_id` 隔离；多表可见写入使用事务；网络、MIME 解析和慢文件 I/O 不持有 SQLite 写锁。
- 密码和未来 Token 只进入系统凭据库；不得进入 SQLite、可迁移目录、前端持久化、日志或错误详情。
- TLS 严格验证系统信任链；明文连接必须由用户明确确认；不得增加忽略证书错误选项。
- HTML 邮件由 Rust 权威清洗并在 sandbox iframe 渲染。不得开放 scripts、forms、same-origin、top-navigation、任意文件或任意网络权限。
- 功能、架构或安全行为变化同步更新 `technical-reference.md`、`architecture.md`、当前 iteration 和新 change；需要长期保留理由的重大取舍新增 ADR。

## 6. 当前同步模型

每账户完整同步只有四种产品时机：首次设定账户、应用启动、账户配置的 1/5/10 分钟周期到期、用户手动收取；`0` 表示仅手动。设置变化只重置计时，不立即同步。持久化邮件待办、Sent/Drafts APPEND 和 Drafts 定向刷新不触发完整账户同步。

同步始终先遍历可选文件夹，以最多 100 UID 为网络 FETCH 批次，但每封邮件头单独原子落库并发布最小事件。当前文件夹前端按事件顺序重读本地视图，100ms 只用于合并增量绘制，不等待整批完成，也没有定时模拟播放。

默认不下载所有正文：打开缺失正文的邮件时按需获取，本地已有原始 EML 时优先离线重建。账户开启“收取邮件全文”后，每个文件夹头部完成后补齐缺失正文；不会把正文回填当成新邮件候选。Drafts APPEND 后的单文件夹定向刷新始终跳过全文阶段。

所有账户共享两个高层网络许可。具体 IMAP Adapter 为每账户提供最多三条按需建立、操作结束即关闭的主动会话预算；完整同步使用两条 worker，为正文、附件、文件夹结构或待办保留第三条，更多操作等待而不取消已有同步。这是有界会话容量，不是长期缓存已登录 Session 的通用连接池。

## 7. 数据、配置、日志和安全内容

用户选择的可迁移目录包含：

```text
.nextmail-data.json
content.sqlite
raw/<hash-prefix>/<hash>
attachments/<hash-prefix>/<hash>
cache/attachment-open/...
```

SQLite schema metadata 当前为版本 24，迁移文件到 `0024`。`.nextmail-data.json` 的 `DataDirectoryMarker.format_version` 当前仍是独立的版本 1；它表示目录标记兼容性，不是 SQLite schema 版本。迁移只允许新增，不修改已经存在的迁移。

系统应用配置目录的 `config/` 包含 `bootstrap.json`、`accounts.json`、`preferences.json`、`reading-preferences.json` 和 `notification-preferences.json`。密码使用服务名 `com.taurusxin.nextmail` 的 Windows Credential Manager/macOS Keychain 条目，配置只保存不透明引用。

日志位于 `<app_local_data_dir>/logs/nextmail.log.YYYY-MM-DD`。Rust panic、稳定错误码、协议/SQLx 原因链、后台任务、窗口/Event 副作用和前端未捕获异常写入统一日志；不得记录 `CommandError.params`、凭据、Token、正文或服务器原始响应。当前没有日志保留/自动清理策略。

HTML 阅读允许安全作者样式、传统表格属性和受限响应式 CSS；移除脚本、表单、嵌入文档、事件属性、外部样式表、CSS 网络资源、危险 URL、动画/变换和固定遮罩。阅读 iframe 只有 `allow-popups`，外链由 Rust 宿主再次校验后交给系统并拒绝在应用 WebView 内创建页面。远程图片默认由 iframe CSP 阻止。

CID 与 `data:image` 只接受经过 media type、解码、文件魔数和大小预算确认的 PNG/JPEG/GIF/WebP/BMP。错误标记为 `application/octet-stream` 的 CID part 仅凭真实文件魔数兼容；只有正文实际引用且成功内联的 part 才从附件区排除。

## 8. 窗口、Capability 与前端状态

窗口类型为：`main`、`composer-*`、`settings`、`accounts`、`definition-*`、`message-preview-*`、`raw-message`、`notification-*`。每类使用独立 Capability；前端没有 Shell、数据库、任意网络、任意文件或任意建窗权限。

`settings`、`accounts`、`raw-message` 是单例；Composer 按草稿、定义窗口按编辑目标、邮件预览按规范邮件 ID 动态创建。普通窗口先隐藏，React 懒加载和首批 Query 完成后显示；加载失败时错误边界也会显示。窗口状态由 Rust 侧 Tauri `window-state` 保存，动态标签映射为 `composer`、`definition`、`message-preview` 类别；通知窗口不保存状态。

Windows 使用无 decorations 的 React 自绘标题栏；macOS 使用 Overlay 和系统交通灯。主窗口/通知保留 `NextMail`，其他业务窗口标题随中文/英文偏好更新。

外观偏好由 TanStack Query 单一数据源管理，各 WebView 从 Rust 持久化值初始化并用窄事件同步，不共享 React 内存状态。默认主题为浅色；已有选择不覆盖。纵向滚动使用 `OverlayScrollArea` 自绘覆盖滑块，不挤压主体；除文件夹列表自动隐藏外，只要可滚动就常驻。

## 9. Composer、草稿、模板和发件

- 草稿保存 Tiptap JSON、HTML、纯文本和 revision；不存在停止输入后的自动保存。
- 关闭可编辑 Composer 时只允许取消、不保存、保存为草稿。保存后排入 Drafts APPEND；服务器确认后定向刷新 Drafts。主侧栏没有第二套本地草稿入口。
- 回复/回复全部/转发的稳定结构为：回复区、签名分隔线、签名、空行、原始邮件分隔线/元数据、完整原文。
- 原文是 `nextmailOriginalMessage` 原子节点中的 `sourceHtml`，不进入会重排表格的 ProseMirror schema；空 sandbox iframe 只在原文/CID 输入变化时重建。
- 切换签名用真实 ProseMirror `nodeSize` 替换稳定签名节点和紧邻分隔节点，不应触及原文。
- 模板和签名可为全局或账户范围；四种场景只配置默认模板，每个范围只有一个默认签名和自动插入开关。
- Composer 图片进入账户隔离的内容寻址附件存储并用 CID 发出；定义编辑窗口的受限小图使用经 Rust 校验的 data URL。远程图片不会因进入 Composer 而被静默下载。
- SMTP 前生成不可变 MIME 和 Message-ID，写入 `raw/` 并建立 `send_job`。重试复用相同 MIME；SMTP 成功后独立 APPEND Sent，归档失败不得再次发送。公开客户端头为 `X-Mailer: NextMail/0.1.0`。

## 10. 开发、验证与交付

日常由用户运行：

```powershell
pnpm tauri dev
```

自动验证基线：

```powershell
pnpm test
pnpm build
Push-Location src-tauri
cargo fmt --all -- --check
cargo test --offline --locked
cargo clippy --offline --locked --all-targets -- -D warnings
Pop-Location
git diff --check
```

第二十一阶段最后记录的基线为前端 33 个测试文件/106 项测试通过，Rust 147 项测试通过，build/fmt/clippy/diff check 通过；Vite 保留既有大 chunk 警告。此记录不是未来补丁的验证结果，新会话修改代码后必须重新执行与风险相称的检查。

不要日常执行 Tauri 完整 bundle，不主动清理 `dist` 或 `src-tauri/target` 增量缓存。临时探针、凭据、日志、截图、测试目录和 coverage 需要清理，正式测试保留。未经用户明确要求，不 commit、push、配置远端、创建 GitHub Workflow/Release、签名或发布。

每一批先在 iteration 明确范围和非目标，再实施、自动验证并交给用户实机验收；用户明确确认通过后才记录验收，并且仍需用户明确要求才提交。额外产品行为、依赖、安全策略或 UI 决策需先确认。依赖优先 MIT/Apache/BSD/ISC，商业/Cloud/Pro 或其他许可证必须先确认。

## 11. 当前已知限制与接手注意事项

- Vite 主入口压缩后仍超过 500 kB，存在大 chunk 警告；没有获授权时不要把全局拆包优化夹入功能阶段。
- 前端尚无 ESLint/Prettier 和分支/PR CI；阶段二十二只在用户授权下增加 `v*` tag 发布 Action，不把普通 push 或 pull request 纳入触发范围。
- 日志尚无保留策略。
- 远程图片代理/缓存、CSS 背景图和 Web Font 未实现。
- 历史 iteration 描述的是当时计划，可能已被后续 change/ADR 修订。例如早期 IDLE 计划不是当前实现事实。
- 工作区可能含用户未提交修改。开始前先检查；与任务冲突时停止说明，不能重置、覆盖或顺手整理无关变动。

## 12. 可复制的新会话提示词

```text
你正在接手 NextMail 项目。仓库目录是 E:\Workspace\Rust\nextmail。不要依赖此前会话或凭记忆猜测状态；以当前检出代码、最近 changes 和仍有效 ADR 为事实来源。

开始工作前：
1. 完整阅读 docs/handoff.md、docs/technical-reference.md、docs/architecture.md、docs/development.md、docs/plans/master-plan.md。
2. 阅读与本次任务相关的最新 docs/iterations、docs/changes，以及 docs/adr 下仍有效 ADR；0001 已被 0006 取代，0007 的同步调度/会话并发分别由 0013/0014 修订。
3. 执行 git status --short 和 git log -3 --oneline --decorate，检查当前 HEAD 与用户未提交修改。有冲突先停下说明，不要 reset、覆盖或清理用户修改。

当前已知基线：NextMail 0.1.0；第二十一、第二十二阶段已验收；第二十二阶段交付双语 README 与 `v*` tag 三平台 GitHub Release 工作流，状态见 `docs/iterations/0022-project-readme-and-release-automation.md`。文档整理时代码 HEAD 为 db4b806，origin/main 为 92b4197，但必须以当前实际检出状态为准。当前没有活动阶段，不要从旧路线自行选择范围外功能。

必须保持：
- Node 依赖只用 pnpm；当前不用 Python，未来需要时只用 uv。
- Rust 只有 src-tauri 单一 Cargo package；不得创建根 Cargo Workspace、根 Cargo.toml/Cargo.lock/target 或业务子 crate。
- React 不直连 SQLite、邮件服务器、任意文件系统或凭据库；所有 IPC 通过 src/app/api.ts 和稳定 DTO；TanStack Query 管理读取模型，事件只失效/重读。
- core 不依赖 Tauri、SQLx、tracing 或协议库；Adapter 由 state.rs 通过 ports 注入；Command 薄并返回稳定 CommandError。
- 密码/Token 只进系统凭据库；账户数据按 account_slot_id 隔离；网络或慢 I/O 不持有 SQLite 写锁。
- TLS 严格验证；HTML 继续由 Rust 清洗并在 sandbox iframe 渲染，不开放 scripts/forms/same-origin/top-navigation/任意文件或任意网络权限。
- 当前同步只有首次账户、应用启动、账户 1/5/10 分钟周期或手动四类时机；没有 IMAP IDLE。每账户最多三条按需 IMAP 会话，完整同步只占两条。
- 功能/架构变化同步维护 technical-reference、architecture、当前 iteration 和新 changes；重大取舍新增 ADR。
- 不开发范围外功能，不自行引入依赖/安全/UI 决策；未经明确要求不 commit、push、配置远端、创建 Workflow/Release、签名或发布。
- 日常不要跑 Tauri bundle，不清理 dist 和 src-tauri/target；用户用 pnpm tauri dev 实机验收。

实施后按风险运行 pnpm test、pnpm build；在 src-tauri 运行 cargo fmt --all -- --check、cargo test --offline --locked、cargo clippy --offline --locked --all-targets -- -D warnings；最后 git diff --check。完成自动验证后提供清晰实机验收步骤，等待我确认。

本次任务：<在这里写入新的需求>
```
