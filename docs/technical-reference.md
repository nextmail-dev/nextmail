# NextMail 当前技术参考

更新时间：2026-07-20

本文描述仓库当前代码已经实现并通过验收的技术状态。阶段进度和后续范围见 `iterations/`，历史变更见 `changes/`，长期架构理由见 `adr/`。

## 1. 产品状态

NextMail 当前版本为 `0.1.0`，目标平台为 Windows 10 22H2+ x64 与 macOS 12+ Intel/Apple Silicon。Windows 是当前主要实机验收平台；macOS 已有平台配置和窗口适配，但未经执行的行为不能宣称通过。

已经实现：

- 首次启动欢迎页、数据目录选择和首个密码账户验证。
- 多个 IMAP/SMTP 密码账户的添加、编辑、重新认证、切换和安全移除。
- IMAP TLS、STARTTLS 和经明确确认的明文连接；SMTP TLS、STARTTLS 和明文连接。
- SQLite 离线邮件、文件夹、正文、附件、草稿、发件任务和待办操作视图。
- 增量同步、Inbox IDLE、无 IDLE 轮询、定时全文件夹对账、断线退避和手动收取。
- 已读、星标、移动、复制、归档、删除的本地乐观更新与持久化重放。
- RFC 2047、MIME 多字符集、IMAP modified UTF-7 文件夹名解析。
- 纯文本和 sandbox iframe 安全 HTML 阅读、远程图片手动/偏好加载。
- 原始 EML、邮件附件按需下载、系统打开、安全另存为。
- 独立富文本写信窗口、三格式草稿、附件、持久化 SMTP 发件、Sent/Drafts APPEND、回复/回复全部/转发。
- 多账户 Supervisor、公平发件调度、账户级同步策略和文件夹角色映射。
- 中文与英文、系统/浅色/深色主题、主题色以及 Windows/macOS 窗口壳。

尚未实现：

- POP3、Google/Microsoft OAuth；当前均为未排期设想。
- 邮件模板、签名模板及身份系统。
- FTS5 全文搜索、会话聚合、统一收件箱。
- 托盘、系统新邮件通知、自动更新与正式发布流水线。
- 联系人、规则、日历、PGP/S-MIME、EML/MBOX 导入导出。

## 2. 技术栈

前端：

- React 19、TypeScript 5.8、Vite 7。
- TanStack Query 5 负责服务端/本地视图缓存和外观偏好单一数据源；前端不再依赖 Zustand。
- react-i18next/i18next 提供 `zh-CN` 与 `en-US`。
- Tailwind CSS 4、CSS Variables、class-variance-authority、Radix Primitives 和源码归属的 shadcn 风格组件层。
- Tiptap/ProseMirror 3 开源组件用于富文本写信。
- Vitest、Testing Library 和 jsdom 负责前端测试。

桌面与后端：

- Tauri 2、Tokio、serde/serde_json、async-trait。
- async-imap 0.11、lettre 0.11、mail-parser 0.11 `full_encoding`、mail-builder 0.4。
- SQLx 0.9 + SQLite WAL + 嵌入式迁移。
- rustls 0.23 + ring + 系统根证书；进程启动时显式安装 CryptoProvider。
- keyring 4.1 连接 Windows Credential Manager/macOS Keychain。
- Ammonia 4 清洗 HTML；SHA-256 用于内容寻址和去重。
- hickory-resolver、reqwest、quick-xml 用于账户自动发现。

仓库只存在一个 Rust package：`src-tauri/Cargo.toml`。根目录没有 Cargo Workspace、`Cargo.toml`、`Cargo.lock` 或 `target`。

## 3. 仓库结构

```text
nextmail/
├─ src/                         React 应用
│  ├─ app/                     启动、API、类型、主题、语言、平台
│  ├─ components/ui/           自有基础组件
│  ├─ components/window/       跨平台窗口标题栏
│  ├─ features/                onboarding/accounts/mail/composer/preferences；mail/hooks 承载选择、事件和分栏状态
│  ├─ locales/                 zh-CN 与 en-US
│  ├─ styles/                  主题、基础、全局和写信样式
│  └─ test/                    前端测试初始化
├─ src-tauri/
│  ├─ capabilities/            main/composer/settings 窄权限
│  ├─ migrations/              SQLx 嵌入式迁移
│  └─ src/
│     ├─ core/                 DTO、稳定错误、ports
│     ├─ application/          首次启动/账户用例、纯草稿组合用例
│     ├─ adapters/             JSON 配置、Keyring、发现、连接测试、系统打开
│     ├─ protocols/            IMAP、MIME、HTML、TLS
│     ├─ storage/              SQLite 子 Repository 与内容存储
│     ├─ commands/             Tauri Command 薄边界
│     ├─ mail_runtime.rs       多账户同步/待办 Supervisor
│     ├─ composer_runtime.rs   草稿与发件 Worker
│     └─ state.rs              具体 Adapter 装配
├─ docs/                       架构、计划、ADR、阶段与变更记录
└─ package.json                pnpm 前端与 Tauri 脚本
```

## 4. 进程与窗口模型

NextMail 使用一个 Tauri 进程：

- `main`：账户、文件夹、邮件列表和阅读器。
- `composer-*`：每个草稿一个独立写信 WebView；可在发送成功时受控销毁。
- `settings`：单例设置 WebView；重复打开只聚焦现有窗口。

Windows 关闭 decorations，由 React 绘制拖动区和窗口按钮。macOS 使用 Overlay 标题栏和系统交通灯，不伪造窗口按钮。每类窗口使用独立 Capability；前端没有 Shell、任意网络、任意文件和数据库权限。

Tauri `setup` 只创建 `AppState`，不阻塞等待同步。React 完成主工作区首帧后调用 `start_background_services`，再启动邮件 Supervisor 和发件 Worker；本地 SQLite 视图先显示，后台落库事件驱动界面逐步刷新。

## 5. Rust 分层与依赖方向

`core` 不依赖 Tauri、SQLx 或具体协议库，包含稳定 DTO、错误、配置 ports、`ImapSyncProvider`、`MailSyncSink` 和同步通知接口。

`application` 组织首次启动、账户生命周期和纯业务用例。账户/Bootstrap/外观/阅读配置通过 trait 注入；回复/转发的收件人、主题、引用块与编辑器内容生成位于 `application/message_composer.rs`。

`adapters` 实现系统或外部边界：

- 原子 JSON 配置存储。
- 系统凭据库。
- 内置服务商、DNS SRV、HTTPS autoconfig。
- IMAP/SMTP 连接验证。
- 附件系统打开或文件管理器显示。

`protocols` 隔离第三方协议库。IMAP 内部拆分为：

- `imap.rs`：Provider、统一连接/登录和同步编排。
- `imap/session.rs`：远端操作、APPEND/替换、IDLE 和 FETCH。
- `imap/parse.rs`：MIME 到领域对象的解析映射。
- `imap/encoding.rs`：文件夹角色和 modified UTF-7。
- `imap/policy.rs`：正文同步窗口计算。
- `tls.rs`：进程级共享 rustls 配置。

`storage` 以同一个 `SqlitePool` 与 `ContentStore` 提供窄子仓库：

- `MailReadRepository`
- `SyncSinkRepository`
- `DraftRepository`
- `SendJobRepository`
- `OperationRepository`
- `MailboxRoleRepository`

`MailRepository` 是共享资源门面，不恢复跨文件的上帝类型。`state.rs` 是组合根，负责注入具体配置 Store、凭据、连接测试、IMAP Provider、Repository Provider 与附件打开器。

## 6. 前端架构

入口根据 URL 查询参数选择主窗口、写信窗口或设置窗口。`src/app/api.ts` 是统一 IPC 客户端，业务组件不直接调用裸 `invoke`。`src/app/types.ts` 与 Rust DTO 保持 camelCase 序列化契约。

读取模型使用 TanStack Query；Rust 事件只触发精确 key 或账户级前缀失效，不直接推送邮件正文。邮件详情 key 固定为：

```text
["message", accountId, mailboxId, messageId]
```

外观偏好统一使用 `appearanceQueryKey`。主窗口、设置窗口和写信窗口各自在所属 WebView 的 QueryClient 中读取 Rust 持久化值；写入先取消相同查询并乐观更新 cache，失败时恢复旧值，成功时以 Rust 返回值覆盖。`appearance-preferences-changed` 只把 DTO 写入当前窗口 cache 并应用 DOM 主题，各 WebView 不共享 React 内存状态。

主工作区由 `useMailboxSelection` 管理账户、文件夹、邮件与当前列表搜索选择，`useMailRuntimeEvents` 管理五类邮件运行时事件及查询失效，`usePaneLayout` 管理折叠、宽度钳制、窗口 resize 和标题栏侧栏宽度令牌。Query key 由邮件 key 工厂集中生成。写信窗口关闭监听按稳定账户/草稿身份只注册一次，通过 ref 读取最新保存函数与编辑状态。

组件层位于 `src/components/ui`，业务页面应优先组合 Button、Select、Dialog、Toast、OverlayScrollArea、ResizeHandle 等生产组件。原生 HTML 只在基础组件或满足语义/可访问性需求时使用，不展示浏览器默认表单外观。

主题由语义 CSS Variables 驱动，支持系统、浅色、深色和多种主题色。Windows 使用 Segoe UI + Microsoft YaHei UI，macOS 使用系统 UI + PingFang SC。语言包缺失时回退英文。

## 7. Command 与 Event 边界

公开 Command 按用途分组：

- 启动与偏好：Bootstrap、数据目录、外观、阅读偏好、后台服务。
- 账户：发现、连接测试、添加、编辑、重新认证、移除、运行状态、最近账户。
- 阅读：文件夹、分页邮件、详情、正文、原始 EML、附件。
- IMAP 写操作：已读、星标、移动、复制、删除、归档、角色映射和重试。
- 写信：打开窗口、草稿 CRUD、附件、远端 Drafts、发件排队和重试。
- 窗口与应用：设置窗口、About 和明确退出。

完整签名以 `src/app/api.ts` 和 `src-tauri/src/commands/mod.rs` 为准。所有失败统一为：

```text
CommandError { code, params, retryable }
```

前端只能看到稳定错误码与本地化参数，不接收密码、服务器原始响应、内部路径或堆栈。

当前事件包括：

- `appearance-preferences-changed`
- `reading-preferences-changed`
- `accounts-changed`、`account-removing`
- `account-runtime-status-changed`
- `sync-progress`、`sync-failed`
- `mailbox-changed`、`message-content-changed`
- `pending-operation-changed`
- `send-job-changed`

邮件事件仅包含账户、文件夹、消息/任务/操作 ID、状态或修订号；界面收到事件后重新读取本地视图。

## 8. 数据与迁移

用户选择的可迁移目录：

```text
.nextmail-data.json
content.sqlite
raw/<hash-prefix>/<hash>
attachments/<hash-prefix>/<hash>
cache/attachment-open/...
```

系统应用配置区的 `config/`：

- `bootstrap.json`：当前数据目录和首次启动状态。
- `accounts.json`：服务器、认证类型、匿名数据槽映射、最近账户和待清理凭据引用。
- `preferences.json`：主题、语言和主题色。
- `reading-preferences.json`：远程图片和附件打开偏好。

密码只存入系统凭据库，服务名为 `com.taurusxin.nextmail`；配置文件只保存不透明 `credential_ref`。

SQLite 数据格式当前为版本 7。主要表：

- `account_slots`、`account_sync_settings`
- `mailboxes`、`mailbox_role_overrides`
- `messages`、`message_locations`、`message_bodies`、`attachments`
- `sync_states`、`remote_image_permissions`
- `drafts`、`draft_attachments`、`send_jobs`
- `pending_operations`

邮件规范记录与远端位置分离，同一邮件可位于多个文件夹。原始 EML 是重新解析来源；正文和附件元数据可重建。迁移只能新增到 `src-tauri/migrations`，已发布迁移不得修改。

## 9. 同步与离线语义

`MailRuntime` 按账户维护一个 Supervisor：

- 同一账户主动操作串行执行；所有账户共享两个网络许可。
- 支持 IDLE 时即时唤醒，5 分钟全文件夹同步兜底；不支持 IDLE 时每 60 秒轮询。
- 网络错误从 2 秒指数退避到最多 5 分钟。
- 首次启动和手动同步显示进度；自动同步静默落库并发送数据变化事件。
- 摘要和正文最多按 100 个 UID 批量 FETCH。
- UIDVALIDITY/UID/MODSEQ 持久化；UIDVALIDITY 改变时重建位置。

已读、星标、移动、复制和删除在一个事务中同时更新本地投影并写入 `pending_operations`。Worker 联网后按序重放；异常退出遗留的 `running` 会恢复。CONDSTORE 冲突会读取最新 MODSEQ 后重放一次，缺少 UIDPLUS 时不做可能误删其他邮件的宽泛 EXPUNGE。

## 10. 草稿与发件

草稿保存 `editor_json`、`html` 与 `plain_text`，使用 revision 做乐观并发控制。完全空白且未修改的草稿可以条件删除，普通草稿不会被误删。

发件流程：

1. Rust 生成包含本机时区 Date 的不可变 MIME。
2. MIME 按 SHA-256 原子保存到 `raw/`。
3. SQLite 创建持久化 `send_job`。
4. SendWorker 按账户内 FIFO、账户间轮转发送；全局最多两封、每账户最多一封。
5. SMTP 成功后只标记发送成功，再独立排队 APPEND Sent，避免归档失败导致重复发信。

临时错误最多自动尝试三次；异常退出的 `sending` 会恢复为 `queued`。本地草稿停止编辑或关闭窗口后排队 APPEND Drafts，使用 `X-NextMail-Draft-ID` 关联远端版本。

## 11. 邮件内容安全

- RFC 2047 头、地址、正文和附件名由 `mail-parser full_encoding` 统一解析。
- HTML 在 Rust 端删除脚本、事件、表单、iframe、危险 URL、外部样式表和 CSS 资源。
- 保留白名单内的行内排版、颜色、尺寸、表格和间距样式。
- 阅读器使用无 scripts/forms/same-origin/top-navigation 的 sandbox iframe。
- 远程图片默认由 iframe CSP 阻止，用户可单次显示或启用设备级自动加载；加载使用 `no-referrer`。
- 当前清洗结果会移除所有超链接 `href`，阅读器尚不能打开邮件外链；保留安全链接、离站确认和系统打开已进入第十阶段计划。
- 已收附件按账户槽验证归属；高风险扩展名只在文件管理器显示，不自动执行。

## 12. 当前已知技术债与限制

- 前端尚无 ESLint/Prettier/CI；是否添加 GitHub Actions 需用户单独确认。
- Vite 主入口压缩后超过 500 kB，写信与设置已动态拆分，但主工作区仍需继续拆包。
- 存储错误对 UI 保持稳定码，但内部诊断日志尚未形成完整 tracing 方案。
- HTML 邮件内 `<style>`、受控外链、CID/内联附件协议和远程图片代理仍未完成；第十阶段已规划样式/外链与回复体验，CID 和代理是否纳入仍需单独确认。

总体阶段顺序见 `plans/master-plan.md`；各阶段范围、进度和验收边界统一记录在 `iterations/`。
