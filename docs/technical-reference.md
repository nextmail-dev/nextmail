# NextMail 当前技术参考

更新时间：2026-08-01

本文描述仓库当前检出代码的技术状态。第二十一、第二十二阶段已经验收，当前没有后续活动阶段；第二十二阶段的双语项目 README 与 tag 驱动三平台 GitHub Release 自动化范围见 `iterations/0022-project-readme-and-release-automation.md`。阶段进度见 `iterations/`，历史变更见 `changes/`，长期架构理由见 `adr/`，跨会话入口见 `handoff.md`。

## 1. 产品状态

NextMail 当前版本为 `0.1.0`，目标平台为 Windows 10 22H2+ x64 与 macOS 12+ Intel/Apple Silicon。Windows 是当前主要实机验收平台；macOS 已有平台配置和窗口适配，但未经执行的行为不能宣称通过。

已经实现：

- 首次启动欢迎页、数据目录选择和首个密码账户验证。
- 多个 IMAP/SMTP 密码账户的添加、编辑、重新认证、切换和安全移除。
- IMAP TLS、STARTTLS 和经明确确认的明文连接；SMTP TLS、STARTTLS 和明文连接。
- SQLite 离线邮件、文件夹、正文、附件、草稿、发件任务和待办操作视图。
- 增量同步、启动同步、按账户选择的手动/1/5/10 分钟定时同步和手动收取。
- 已读、星标、移动、复制、归档、删除的本地乐观更新与持久化重放。
- RFC 2047、MIME 多字符集、IMAP modified UTF-7 文件夹名解析。
- 纯文本和 sandbox iframe 安全 HTML 阅读、远程图片手动/偏好加载。
- 原始 EML、邮件附件按需下载、系统打开、安全另存为。
- 独立富文本写信窗口、关闭时显式确认的三格式草稿、附件、持久化 SMTP 发件、Sent/Drafts APPEND、回复/回复全部/转发。
- 全局或账户范围的富文本邮件模板与签名库 CRUD、四种写信场景默认模板、单一默认签名与自动插入偏好、受控变量渲染和 Composer 稳定节点。
- 多账户 Supervisor、公平发件调度、账户级同步策略和文件夹角色映射。
- 在线 IMAP 文件夹创建、重命名、层级移动、删除和批量全部已读，以及按账户保存在 SQLite 的本地文件夹排序。
- 全服务器邮件头部优先同步、打开时按需正文、逐封本地可见性、手动正文阶段进度与受限 CID 图片离线阅读。
- 每封邮件原子落库后通知当前文件夹串行重新读取本地视图；已有本地首屏立即显示，后台同步期间手动收信禁用且 Rust 拒绝重复同步竞态。
- 当前账户、当前文件夹范围的 SQLite FTS5 本地全文搜索，覆盖主题、地址、预览、纯文本正文和附件名。
- 全局/账户/文件夹分层的新邮件通知偏好、首次同步抑制和 NextMail 自有桌面通知窗口；层叠/覆盖、数量、时间及点击定位已在 Windows 10 22H2+ 与 macOS 12+ 验收。
- 中文与英文、系统/浅色/深色主题、主题色以及 Windows/macOS 窗口壳。
- `v*` Git tag push 触发的 GitHub Release 工作流，为 Windows x64、macOS Universal 和 Linux x64 构建预览 bundle；三个构建全部成功后才公开 Release。

尚未实现：

- POP3、Google/Microsoft OAuth；当前均为未排期设想。
- 会话聚合、跨账户搜索、统一收件箱。
- 托盘、系统通知中心集成、自动更新与正式发布硬化；当前自有通知窗口不进入系统通知历史，也不宣称系统勿扰模式集成。现有 tag 工作流不包含 Windows 正式代码签名、Apple Developer 签名/公证或自动更新元数据。
- 联系人、规则、日历、PGP/S-MIME、EML/MBOX 导入导出。

## 2. 技术栈

前端：

- React 19、TypeScript 5.8、Vite 7。
- TanStack Query 5 负责服务端/本地视图缓存和外观偏好单一数据源；前端不再依赖 Zustand。
- react-i18next/i18next 提供 `zh-CN` 与 `en-US`。
- Tailwind CSS 4、CSS Variables、class-variance-authority、Radix Primitives 和源码归属的 shadcn 风格组件层。
- Tiptap/ProseMirror 3 开源组件用于富文本写信；官方 MIT `@tiptap/extension-table` 3.27.3 提供普通撰写表格 schema，引用原文不经过该模型。MIT CodeMirror 6 提供 HTML 源码与实时预览双栏。
- Vitest、Testing Library 和 jsdom 负责前端测试。

桌面与后端：

- Tauri 2、Tokio、serde/serde_json、async-trait。
- async-imap 0.11、lettre 0.11、mail-parser 0.11 `full_encoding`、mail-builder 0.4。
- SQLx 0.9 + SQLite WAL + 嵌入式迁移。
- rustls 0.23 + ring + 系统根证书；进程启动时显式安装 CryptoProvider。
- keyring 4.1 连接 Windows Credential Manager/macOS Keychain。
- Ammonia 4 清洗 HTML 结构，`cssparser 0.37` 解析并重建安全 CSS 子集；SHA-256 用于内容寻址和去重。
- hickory-resolver、reqwest、quick-xml 用于账户自动发现。

仓库只存在一个 Rust package：`src-tauri/Cargo.toml`。根目录没有 Cargo Workspace、`Cargo.toml`、`Cargo.lock` 或 `target`。

## 3. 仓库结构

```text
nextmail/
├─ src/                         React 应用
│  ├─ app/                     启动、API、类型、主题、语言、平台
│  ├─ components/ui/           自有基础组件
│  ├─ components/window/       跨平台窗口标题栏
│  ├─ features/                onboarding/accounts/mail/composer/preferences/notifications；mail/hooks 承载选择、事件和分栏状态
│  ├─ locales/                 zh-CN 与 en-US
│  ├─ styles/                  主题、基础、全局和写信样式
│  └─ test/                    前端测试初始化
├─ src-tauri/
│  ├─ capabilities/            main/composer/settings/accounts/message-preview/raw-message/definition/notification 窄权限
│  ├─ migrations/              SQLx 嵌入式迁移
│  └─ src/
│     ├─ core/                 DTO、稳定错误、ports
│     ├─ application/          首次启动/账户用例、纯草稿组合用例
│     ├─ adapters/             JSON 配置、Keyring、发现、连接测试、系统打开
│     ├─ protocols/            IMAP 编排/连接/会话预算/操作、MIME、HTML、TLS
│     ├─ storage/              SQLite 读取/同步写入/草稿/发件任务等窄 Repository 与内容存储
│     ├─ commands/             Tauri Command 薄边界
│     ├─ mail_runtime.rs       多账户查询与同步 Supervisor 门面；子模块承载正文、待办和事件支撑
│     ├─ composer_runtime.rs   Composer 门面；子模块承载定义管理和持久化发件调度
│     ├─ notification_runtime.rs 临时通知窗口、层叠/覆盖与超时调度
│     └─ state.rs              具体 Adapter 装配
├─ docs/                       架构、计划、ADR、阶段与变更记录
└─ package.json                pnpm 前端与 Tauri 脚本
```

## 4. 进程与窗口模型

NextMail 使用一个 Tauri 进程：

- `main`：账户、文件夹、邮件列表和阅读器。
- `composer-*`：每个草稿一个独立写信 WebView；可在发送成功时受控销毁。
- `settings`：单例设置 WebView；重复打开只聚焦现有窗口。
- `accounts`：单例账户管理 WebView；主窗口账户菜单和无账户空状态只请求 Rust 创建或聚焦。
- `definition-*`：按模板/签名编辑目标创建的独立富文本 WebView；仅通过稳定定义 Command 和最小事件工作。
- `message-preview-*`：按规范邮件 ID 复用的独立阅读 WebView；双击邮件或菜单请求由 Rust 核验账户、文件夹和邮件位置后创建。
- `raw-message`：单例原始邮件 WebView；重复查看时只以账户/邮件 ID 事件切换目标，原始 EML 由窗口通过稳定 Command 读取。
- `notification-*`：受限的瞬时新邮件窗口；不进入任务栏、不主动抢焦点，也不保存窗口状态。

Windows 关闭 decorations，由 React 绘制拖动区和窗口按钮。macOS 使用 Overlay 标题栏和系统默认交通灯位置，不伪造窗口按钮或硬编码交通灯坐标。主窗口和瞬时通知保留品牌标题 `NextMail`；Composer、设置、账户管理、独立邮件预览、邮件原文及模板/签名编辑窗口的原生标题和 Windows 自绘标题均使用当前中文/英文偏好，语言切换后 Rust 同步更新已打开窗口的原生标题。每类窗口使用独立 Capability；前端没有 Shell、任意网络、任意文件和数据库权限。`message-preview-*` 只获得自身窗口控制和稳定业务 Command；其中的邮件外链继续由 Rust 宿主的新窗口回调复验并交给系统关联程序，外部网页不会在预览 WebView 内加载。

Tauri 官方 `window-state` 插件只在 Rust 宿主注册，保存尺寸、位置与最大化状态到系统应用配置目录。`main`、`settings`、`accounts`、`raw-message`、`composer`、`message-preview` 和 `definition` 窗口类型分别恢复状态；动态 `composer-*`、`message-preview-*` 与 `definition-*` 分别映射到对应公共类别，避免按草稿、邮件或定义 ID 无限累积记录。`notification-*` 由插件 filter 明确排除，不持久化瞬时内容或几何状态。普通窗口创建时先隐藏并居中；有历史状态时插件恢复后显示，没有历史状态时保持居中默认尺寸。React 不读写窗口状态文件，也不获得任意建窗权限。

`NotificationRuntime` 由 `state.rs` 注入 `MailRuntime`。候选只在账户同步完整成功后进入调度；窗口按主窗口所在显示器的物理工作区和缩放因子从右下角向上排列，显示器高度不足时钳制实际可见数量。覆盖模式复用同一窗口并以 generation 使旧超时失效，层叠模式达到上限时淘汰最早窗口。偏好改变关闭当前临时通知，账户移除只清理该账户窗口。

Tauri `setup` 创建 `AppState`，并从既有平台窗口配置显式创建带外链新窗口处理器的主窗口，不阻塞等待同步。React 完成主工作区首帧后调用 `start_background_services`，再启动邮件 Supervisor 和发件 Worker；本地 SQLite 视图先显示，后台落库事件驱动界面逐步刷新。

## 5. Rust 分层与依赖方向

`core` 不依赖 Tauri、SQLx、`tracing` 或具体协议库，包含稳定 DTO、错误、配置 ports、`ImapSyncProvider`、`MailSyncSink`、`ExternalLinkOpener` 和同步通知接口。进程启动时由外层为 `CommandError` 安装只读观察器，只记录稳定错误码、retryable 与 Rust 调用位置，不接收或记录错误参数。

`application` 组织首次启动、账户生命周期和纯业务用例。账户/Bootstrap/外观/阅读/通知配置通过 trait 注入；回复/转发的收件人、主题、稳定回复/原文节点与三格式初始内容生成位于 `application/message_composer.rs`。模板/签名名称、主题长度、变量白名单、按上下文转义和初始三格式组合位于 `application/composition_definitions.rs`，不进入 SQLx Repository。

`adapters` 实现系统或外部边界：

- 原子 JSON 配置存储。
- 系统凭据库。
- 内置服务商、DNS SRV、HTTPS autoconfig。
- IMAP/SMTP 连接验证。
- 附件系统打开或文件管理器显示。
- 已校验外链的系统浏览器或邮件程序打开。

`protocols` 隔离第三方协议库。IMAP 内部拆分为：

- `imap.rs`：文件夹同步、UID 差集、Flags 对账和头部抓取编排。
- `imap/provider.rs`：`ImapSyncProvider` 具体实现与按操作会话租约。
- `imap/connection.rs`：TCP/TLS/STARTTLS、登录和传输超时。
- `imap/session_budget.rs`：账户级有界会话预算。
- `imap/path_lock.rs`：账户级 mailbox 路径读写锁；普通协议操作共享读锁，结构变更独占写锁。
- `imap/session.rs`：邮件与文件夹远端操作、APPEND/替换和单封 FETCH。
- `imap/parse.rs`：MIME 到领域对象的解析映射。
- `imap/encoding.rs`：文件夹角色和 modified UTF-7 双向转换。
- `imap/timeout.rs`：每次成功收发后重置的读写超时流。
- `tls.rs`：进程级共享 rustls 配置。

`storage` 以同一个 `SqlitePool` 与 `ContentStore` 提供窄子仓库：

- `MailReadRepository`
- `SyncSinkRepository`
- `DraftRepository`
- `SendJobRepository`
- `OperationRepository`
- `MailboxRepository`
- `MailboxRoleRepository`
- `CompositionDefinitionRepository`

`MailRepository` 是共享资源门面，不恢复跨文件的上帝类型。同步写入实现位于 `message_sync_repository.rs`，文件夹上下文/结构投影/本地顺序位于 `mailbox_repository.rs`，发件任务持久化位于 `draft_repository/send_job_repository.rs`；这些窄仓库共享同一 SQLx pool 和内容存储。`state.rs` 是组合根，负责注入具体配置 Store、凭据、连接测试、IMAP Provider、Repository Provider、附件打开器与外链打开器。

## 6. 前端架构

入口根据 URL 查询参数选择主窗口、写信窗口、设置窗口、账户管理窗口、模板/签名定义窗口、独立邮件预览、原始邮件窗口或按需加载的通知窗口。设置、账户管理、定义编辑、独立邮件预览、原始邮件、Composer 和通知窗口继续按窗口动态加载；Rust 隐藏创建 WebView，React 完成代码块加载和首批 Query 后才显示，Query 失败时显示错误状态，普通独立窗口的 React 错误边界也会主动显示，不再把中间加载动画暴露给用户。独立预览先加载文件夹摘要和精确邮件详情，再显示并复用既有 `MessageViewer`。`src/app/api.ts` 是统一 IPC 客户端，业务组件不直接调用裸 `invoke`。`src/app/types.ts` 与 Rust DTO 保持 camelCase 序列化契约。

读取模型使用 TanStack Query；Rust 事件只触发精确 key 或账户级前缀失效，不直接推送邮件正文。邮件详情 key 固定为：

```text
["message", accountId, mailboxId, messageId]
```

外观偏好统一使用 `appearanceQueryKey`。每个主窗口和独立 WebView 都在根级 Appearance Bridge 中读取 Rust 持久化值，设置与写信功能复用同一 Query cache；写入先取消相同查询并乐观更新 cache，失败时恢复旧值，成功时以 Rust 返回值覆盖。`appearance-preferences-changed` 只把 DTO 写入当前窗口 cache 并应用 DOM 主题，各 WebView 不共享 React 内存状态。这样账户管理等独立窗口首次打开就使用已保存主题色，而不是等待下一次偏好事件。

主工作区由 `useMailboxSelection` 管理账户、文件夹、邮件、搜索输入与通知目标选择，`useMailRuntimeEvents` 管理七类邮件运行时/定位事件及查询失效，`useMailboxActions` 管理文件夹结构 mutation 与 Query 失效，`usePaneLayout` 管理折叠、宽度钳制、窗口 resize 和标题栏侧栏宽度令牌。Query key 由邮件 key 工厂集中生成：普通列表使用 `['messages', accountId, mailboxId]`，当前文件夹搜索只有按 Enter 或点击搜索按钮后才把输入草稿提交为 `['messages', accountId, mailboxId, 'search', query]`；邮件事件失效文件夹或账户前缀即可同时刷新相关搜索。账户运行状态事件会立即刷新 runtime Query，使后台同步期间手动收信保持禁用；当前文件夹收到连续 `mailbox-changed` 时按事件顺序串行重新读取，后一封不会取消前一封刷新，也没有前端定时模拟播放。通知点击先由 Rust 核验目标，为仍存在的目标邮件排入已读更新，再通过 `open-mail-location` 切换账户/文件夹并选择消息；失效消息只保留文件夹选择。邮件列表向工作区报告当前可见顺序，删除、归档或移动成功后用纯选择函数优先选中下一封、末尾回退上一封；再次单击当前行会清除选择。列表滚动视口以账户/文件夹为身份，切换后重新挂载并回到顶部；双击邮件或菜单“在新窗口中打开”只调用 `src/app/api.ts` 的窄建窗命令。Drafts 文件夹隐藏回复、回复全部和转发，保留继续编辑和通用操作。普通列表和搜索都使用相同的 50 封游标分页；设备级阅读偏好默认允许在自绘滚动视口接近底部时调用 TanStack Query `fetchNextPage`，关闭后恢复显式“加载更多”按钮。邮件和文件夹右键菜单都只复用稳定窄命令，不维护第二套后端操作。写信窗口关闭监听按稳定账户/草稿身份只注册一次，通过 ref 读取最新保存函数与编辑状态；关闭可编辑窗口只显示取消、不保存、保存为草稿三个显式动作，不再由计时器保存。Composer Bootstrap 只返回当前账户可见的定义摘要；用户选择定义后由 Rust 使用当前收件人上下文渲染，React 只负责把返回的稳定节点内容交给 Tiptap。构造 Composer `srcdoc` 前还有一层只删除主动元素和 `on*` 属性的防御，避免历史内容触发 sandbox 拦截日志；原始邮件 NodeView 会缓存已渲染输入，无关的签名/模板事务不会重设 iframe。Rust 持久化清洗仍是权威边界。

侧栏顶部账户身份始终打开 Radix 账户菜单，单账户和多账户行为一致；名称和邮箱在固定侧栏宽度内完整换行，头像与菜单箭头保持固定尺寸。账户项之后的分割线提供账户管理入口。账户列表、添加和详情面板由独立 `accounts` 窗口承载，继续使用 `['accounts']`、`['account-runtimes']` 等 Query 失效和 `src/app/api.ts` 窄命令；主窗口不再渲染账户管理弹层。独立设置窗口不维护重复账户类别；移除最后一个账户后，主窗口空状态打开同一个独立管理窗口，不回退到已经完成的数据目录向导。

组件层位于 `src/components/ui`，业务页面应优先组合 Button、Select、Dialog、Toast、OverlayScrollArea、ResizeHandle 等生产组件。原生 HTML 只在基础组件或满足语义/可访问性需求时使用，不展示浏览器默认表单外观。应用纵向滚动区隐藏 WebView 原生滚动条并统一使用 `OverlayScrollArea` 的自绘覆盖滑块，不使用 `scrollbar-gutter`，滑块落在容器既有 padding/margin 内且不挤压主体。默认只要内容可滚动就常驻滑块；文件夹列表是唯一显式启用 `autoHide` 的例外，在 hover 或键盘焦点进入时显示。邮件列表的透明拖动命中区加宽并内收于列表 padding，避免与分栏 ResizeHandle 竞争且不改变列表项主体宽度。

主题由语义 CSS Variables 驱动，支持系统、浅色、深色和多种主题色；未保存外观偏好时默认使用浅色，已有用户选择不被迁移或覆盖。外观设置用三张带可访问 radio 语义的预览卡选择系统/浅色/深色，主题色仍使用独立色板；设置分类、文件夹和账户管理主操作统一使用主题色派生的背景或前景令牌。Windows 使用 Segoe UI + Microsoft YaHei UI，macOS 使用系统 UI + PingFang SC。语言包缺失时回退英文。

## 7. Command 与 Event 边界

公开 Command 按用途分组：

- 启动与偏好：Bootstrap、数据目录、外观、阅读偏好、分层通知偏好、后台服务。
- 通知窗口：与调用窗口 label 绑定的通知 Bootstrap、关闭和激活；React 不创建窗口或决定最终定位目标。
- 账户：发现、连接测试、添加、编辑、重新认证、移除、运行状态、最近账户。
- 阅读：文件夹、分页邮件、当前文件夹本地搜索、详情、正文、独立预览、原始 EML 与附件。邮件外链不经过前端 Command；主窗口和独立预览的宿主新窗口回调都在 Rust 内复验后直接调用系统关联程序。
- 账户同步设置：手动/1/5/10 分钟自动同步间隔，以及默认关闭的“收取邮件全文”。阶段十二遗留的正文时间与“始终下载非收件箱正文”数据库列为兼容旧数据继续保留，但已不在 DTO、设置 UI 或同步决策中使用；间隔变更只重置 Supervisor 计时，不立即执行同步，全文开关从下一次完整同步开始生效。
- IMAP 邮件写操作：已读、星标、移动、复制、删除、归档、角色映射和重试。
- IMAP 文件夹写操作：创建、重命名、层级移动、删除、全部标为已读，以及本地文件夹排序。
- 写信：打开窗口、显式保存或丢弃编辑会话、草稿 CRUD、普通附件、经校验的选择/粘贴内嵌图片、安全富 HTML 粘贴清洗、远端 Drafts、发件排队和重试。
- 模板、签名与规则：按全局或账户范围管理富文本定义、四种场景模板规则及单一默认签名偏好，按当前账户渲染定义；账户 ID 在 Rust 内转换为匿名数据槽。
- 窗口与应用：设置、账户管理、模板/签名定义、独立邮件预览、原始邮件窗口，About 和明确退出。

完整签名以 `src/app/api.ts` 和 `src-tauri/src/commands/mod.rs` 为准。所有失败统一为：

```text
CommandError { code, params, retryable }
```

前端只能看到稳定错误码与本地化参数，不接收密码、服务器原始响应、内部路径或堆栈。

当前事件包括：

- `appearance-preferences-changed`
- `reading-preferences-changed`
- `notification-preferences-changed`
- `accounts-changed`、`account-removing`
- `account-runtime-status-changed`
- `sync-progress`、`sync-failed`
- `mailbox-changed`、`message-arrived`、`message-content-changed`、`message-body-progress`
- `pending-operation-changed`
- `send-job-changed`
- `new-mail-candidate`
- `notification-content-changed`、`open-mail-location`
- `raw-message-location-changed`、`message-preview-location-changed`
- `composition-definitions-changed`

普通邮件事件仅包含账户、文件夹、消息/任务/操作 ID、状态、阶段进度或修订号；界面收到事件后重新读取本地视图。`message-body-progress` 不携带正文，只描述用户手动请求的准备、下载、处理、更新与完成阶段。`new-mail-candidate` 是 `NotificationRuntime` 的最小输入，只包含公开账户/文件夹/消息 ID、首个发件人姓名和地址及主题。覆盖窗口通过定向 `notification-content-changed` 接收相同最小展示 DTO；点击后 Rust 重新核验本地位置，再定向向主窗口发送不含正文的 `open-mail-location`。这些事件都不包含正文、预览、附件、内部路径、凭据或服务器错误。

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
- `reading-preferences.json`：远程图片、附件打开和邮件列表自动分页偏好。
- `notification-preferences.json`：全局、账户和文件夹通知开关，以及层叠/覆盖、最多层叠数量和展示时间；只保存公开 ID。

密码只存入系统凭据库，服务名为 `com.taurusxin.nextmail`；配置文件只保存不透明 `credential_ref`。

SQLite 数据格式当前为版本 24。主要表：

- `account_slots`、`account_sync_settings`
- `mailboxes`、`mailbox_role_overrides`
- `messages`、`message_locations`、`message_bodies`、`attachments`
- `message_search`（FTS5 可重建搜索视图）
- `sync_states`、`remote_image_permissions`
- `drafts`、`draft_attachments`、`send_jobs`
- `mail_templates`、`mail_signatures`、`composition_scene_rules`、`signature_preferences`
- `pending_operations`

邮件规范记录与远端位置分离，同一邮件可位于多个文件夹。原始 EML 是重新解析来源；正文、附件元数据和 FTS 索引可重建。迁移 0016 曾增加非收件箱正文偏好及动作草稿编辑标记，其中正文偏好列现仅为数据兼容保留；迁移 0017 新增默认签名偏好，并把既有场景签名引用按新建、回复、回复全部、转发顺序收敛成单一默认值；迁移 0018 为匿名账户槽增加通知同步基线，并把已经成功同步过的既有账户标记为就绪；迁移 0019 为每个匿名账户槽增加默认 1 分钟、且只允许手动/1/5/10 分钟的同步间隔；迁移 0020 为 `mailboxes` 增加可空的 `local_sort_order`，未排序账户继续使用既有角色/名称顺序，首次拖拽后保存完整账户顺序；迁移 0021 失效旧安全 HTML 缓存，使标准 CID/data 图片策略从本地原始 EML 离线重建；迁移 0022 将数据格式升级至 22，再次失效已由实机应用 0021 的缓存，使误标为 `application/octet-stream` 的 CID 图片通过文件魔数兼容重建，并在同一事务移除已成功内联的重复附件元数据；迁移 0023 将数据格式升级至 23，再次失效安全 HTML 缓存，使本地原始 EML 中经过完整文件头验证的 BMP 按新规则重建；迁移 0024 将数据格式升级至 24，为账户增加默认关闭的全文同步开关，不复用 0016 的废弃列。迁移只能新增到 `src-tauri/migrations`，已发布迁移不得修改。

`message_search` 在数据格式版本 15 的迁移中回填现有邮件，并由数据库触发器随主题/地址/预览、正文纯文本和附件名维护。三字及以上输入使用转义后的字面 trigram FTS 查询；一至两个 Unicode 字符在同一索引存储列上执行受限字面扫描。前端不再以定时防抖自动提交搜索：输入文本只是本地草稿，只有按 Enter 或点击搜索按钮才更新 TanStack Query key 并调用 Rust；清空输入立即撤销已提交关键词并恢复普通列表。查询先由 Rust 将公开账户 ID 解析为匿名数据槽，再同时按 `account_slot_id`、`mailbox_id` 和可见邮件位置约束；HTML 标记、原始 EML、凭据和内部路径不进入索引。结果沿用普通邮件列表 DTO、日期游标和时间倒序，不做相关度排序。

## 9. 同步与离线语义

`MailRuntime` 按账户维护一个 Supervisor：

- 所有账户共享两个高层网络操作许可；具体 `AsyncImapProvider` 另为每个账户维护最多 3 条主动 IMAP 会话的预算。完整同步只租用 2 条 worker 会话，始终为正文、附件、文件夹结构操作或持久化待办保留第 3 条容量；每次操作按需连接并在结束后关闭，不跨 Provider 边界暴露或长期缓存 `async-imap::Session`。
- 完整同步只有四类入口：账户首次设定后启动 Supervisor、应用启动后的首次同步、账户配置的 1/5/10 分钟计时到期，以及用户手动收取；“仅手动”不启动周期计时。
- 不再使用 Inbox IDLE、无 IDLE 轮询、固定 5 分钟兜底或秒级同步失败重试；失败后等待下一次已配置间隔或用户手动重试。
- 文件夹 CREATE/RENAME/DELETE 与批量全部已读是显式在线交互操作，不触发新的完整同步入口，也不进入消息待办队列。运行时先读取短生命周期本地上下文，释放数据库访问后租用交互会话，服务器成功后再以事务更新本地投影；不会持有 SQLite 写锁等待网络。Adapter 按账户用弱引用 mailbox 路径读写锁协调协议操作：完整同步、正文、消息待办和 APPEND/草稿替换共享读锁，结构变更/全部已读取写锁，既避免路径与 Flags 竞态，也不把既有读取重新串行化。重命名/层级移动通过 IMAP `RENAME` 同时改写本地父级及其下级路径并保留 mailbox ID，本地排序则完全不修改服务器路径。
- 草稿 APPEND 在服务器确认且待办持久完成后，通过 core 的单文件夹同步目标再次租用现有预算会话，只执行映射 Drafts 的既有 `sync_folder` 路径，并继续逐封落库、发出列表/文件夹失效事件。该定向刷新不改变 Supervisor 的启动、周期或手动同步入口，不报告完整同步进度，也不持有 SQLite 锁等待网络；若刷新失败，只记录错误并由下一次常规同步修复本地视图，不重放已经成功的 APPEND。
- 首次启动和手动同步显示进度；自动同步静默落库并发送数据变化事件。
- 可见同步进度携带当前 IMAP 文件夹按服务器层级分隔符提取的末级 Unicode 显示名，侧栏显示“正在同步文件夹 ……”；不会展示完整父级路径，事件也不携带正文、地址或其他邮件内容。
- 完整同步覆盖服务器全部可选文件夹和 UID，但只按最多 100 个 UID 的网络批次 FETCH 邮件头；两个 worker 把 UID 分成互不重叠的集合。每封头部独立落库后立即发布 `message-arrived`，当前文件夹按 100ms 窗口合并增量 UI 更新，不等待整批完成。
- 完整同步默认不自动下载或回填正文。用户打开缺少正文的邮件时才通过保留的账户会话容量按需 FETCH 完整 MIME；本地已有原始 EML 时优先离线重新解析。账户在管理窗口显式开启“收取邮件全文”后，每个可选文件夹先完成逐封头部落库，再按 `body_availability` 查询当前 UIDVALIDITY 下的缺失正文，并复用两条同步 worker 会话逐封 FETCH 完整 MIME。网络与 blocking MIME 解析不持有 SQLite 锁，只有正文 UPSERT 进入短暂串行写区；第三条会话容量仍可并行服务交互请求，正文回填不形成新邮件候选。单文件夹 Drafts 定向刷新始终跳过全文阶段。正文完成仍只用最小事件触发 Query 重读。
- UIDVALIDITY/UID/MODSEQ 持久化；UIDVALIDITY 改变时重建位置。
- `MailSyncSink::upsert_message` 只在新建远端位置时形成候选；正文回填和重复同步不会重复产生。新账户首次完整同步、首次发现的文件夹和 UIDVALIDITY 重建均抑制历史候选，只有整个账户同步成功后才持久化基线。后续候选在同步成功后按全局、账户、文件夹偏好过滤，并以账户/文件夹/消息去重；默认只允许 Inbox，且仅处理未读新邮件。

已读、星标、移动、复制和删除在一个事务中同时更新本地投影并写入 `pending_operations`。Worker 联网后按序重放；异常退出遗留的 `running` 会恢复。CONDSTORE 冲突会读取最新 MODSEQ 后重放一次，缺少 UIDPLUS 时不做可能误删其他邮件的宽泛 EXPUNGE。

## 10. 模板、签名、草稿与发件

`mail_templates` 和 `mail_signatures` 保存 Tiptap `editor_json`、HTML 与纯文本。`account_slot_id IS NULL` 表示全局定义，非空值表示只属于对应匿名账户槽；列表按名称稳定排序。每项使用 revision 做乐观更新与删除校验，前端只能通过 `src/app/api.ts` 的稳定 DTO 管理，不能读取数据槽或 SQLite。

数据格式版本 9 新增 `composition_scene_rules`。全局和每个匿名账户槽都可以为 `new`、`reply`、`reply_all`、`forward` 保存模板引用；账户没有显式记录时继承全局模板规则，显式账户规则优先。数据格式版本 17 新增 `signature_preferences`：每个范围只有一个默认签名和自动插入开关，账户没有显式记录时继承全局偏好，创建某范围第一个签名时会在同一事务内自动设为默认。引用使用外键和作用域校验，所有偏好写入使用 revision 防止陈旧覆盖。

变量白名单为 `sender_name`、`sender_email`、`recipient_name`、`recipient_email`、`date`。定义保存时拒绝未知变量；插入时缺少收件人姓名等上下文会返回稳定错误。Rust 分别渲染主题、Tiptap 文本节点、HTML 和纯文本：HTML 变量值进行实体转义，主题移除换行，日期按当前界面语言使用本机日期。

设置窗口“写信”分类提供范围切换、四种默认模板规则、签名列表、默认标记、“设为默认”和“自动为邮件选择默认签名”；新增/编辑定义会打开可记忆大小和位置的独立富文本窗口，设置窗口只通过稳定 IPC 请求打开，并由最小 `composition-definitions-changed` 事件失效对应 Query。独立窗口自行按公开账户 ID 和定义 ID 加载、保存，名称/主题保持内容高度，长正文通过不占主体宽度的统一自绘覆盖滑块滚动。Composer 可显式选择当前账户可见的全局/账户定义；模板与签名分别写入 `nextmailTemplate` / `nextmailSignature` 块节点，并把定义 ID 同步到 HTML 的 `data-nextmail-*-id` 属性。签名节点只保留稳定语义边界，不附带引用线、底色、内边距或圆角。切换通过 ProseMirror 当前文档位置完整替换同类节点，不用序列化 JSON 估算区间；用户删除签名后保存和重开不会自动恢复。初始新建或回复/转发创建时才解析场景模板和签名偏好；关闭自动插入后仍可在 Composer 手动选择签名，远端既有草稿不重新套用默认值。回复/转发另使用 `nextmailReply`、`nextmailSignatureDivider` 与 `nextmailOriginalMessage` 固定边界，默认顺序为回复区、签名分隔线、默认签名、空行、原始邮件分隔线/发件人/发件时间/收件人/主题元数据和正文；模板只进入回复区，签名始终插在原文前，原始邮件元数据使用 `13px` 紧凑字号。动作主题按当前语言添加 `回复：` / `转发：` 或 `Re: ` / `Fwd: `，场景模板主题也执行同一防重复前缀规则。

Composer、签名和模板共用 Tiptap 的结构化 WYSIWYG 与 CodeMirror 6 HTML/预览双栏。`DraftContent.html` 是源码模式的权威文本：只进入、修改、退出源码不会先经 `editor.getHTML()` 重写，因此根级裸文本和源码格式不会无故获得 `<p>`；从源码返回 WYSIWYG 只构建可编辑投影，后续真的发生可视化编辑时才按受支持 schema 重新序列化。编辑 schema 额外保留纯内联 `div`，以及安全 HTML 中图片的 class、ID、style、width/height、对齐、边框和旧式间距属性；画布不再用响应式 CSS 覆盖作者图片尺寸。该边界不承诺任意未知或无效 HTML 经 WYSIWYG 编辑仍逐字无损，保存仍进入既有 Rust 再清洗、远端 Drafts 与 MIME 流程。共用工具栏可设置、更新和移除链接；前端先按与 Rust 相同的 `http`/`https`/`mailto`、无凭据、无控制字符规则校验，编辑器内不直接打开链接。

草稿保存 `editor_json`、`html` 与 `plain_text`，使用 revision 做乐观并发控制。Composer 把发件人显示为只读地址标签；To/Cc/Bcc 在空格、回车、逗号、分号或失焦时即时校验并生成标签，空输入退格会把末尾标签恢复为可编辑文本。编辑过程不再自动保存或自动排入远端 Drafts；关闭可编辑窗口时，用户可取消、明确不保存并删除本地编辑会话，或明确保存最新收件人、主题和三格式正文后排入 Drafts APPEND。无效的待提交地址会阻止保存和发送并保留窗口。新建、回复/回复全部/转发以及服务器草稿都沿用相同关闭确认；远端导入草稿选择不保存时不会删除服务器原件。

发件流程：

1. Rust 生成包含本机时区 Date 和公开 `X-Mailer: NextMail/0.1.0` 客户端标识的不可变 MIME；普通正文使用 `multipart/alternative`，CID 图片位于 `multipart/related`，普通附件再包入外层 `multipart/mixed`。
2. MIME 按 SHA-256 原子保存到 `raw/`。
3. SQLite 创建持久化 `send_job`。
4. SendWorker 按账户内 FIFO、账户间轮转发送；全局最多两封、每账户最多一封。
5. SMTP 成功后只标记发送成功，再独立排队 APPEND Sent，避免归档失败导致重复发信。

临时错误最多自动尝试三次；异常退出的 `sending` 会恢复为 `queued`。只有用户在关闭窗口时明确选择“保存为草稿”，本地编辑内容才排队 APPEND 到映射的 Drafts，并使用 `X-NextMail-Draft-ID` 关联远端版本；主侧栏不再展示独立的本地草稿下拉入口。

## 11. 邮件内容安全

- RFC 2047 头、地址、正文和附件名由 `mail-parser full_encoding` 统一解析。
- HTML 在 Rust 端删除脚本、事件、表单、iframe、危险 URL 和外部样式表。`<style>` 与行内声明都通过 CSS parser 重建，只保留展示属性、普通/属性选择器、受限 An+B 的 `nth-child` / `nth-last-child` / `nth-of-type` / `nth-last-of-type` 及受控响应式/深色媒体查询；`class`、`id` 和传统表格邮件的安全宽高/间距/对齐/背景色/字体属性继续保留。
- CSS `url()`、`@import`、`@font-face`、其他 at-rule、未知函数、固定遮罩、动画和变换继续移除；背景、字体或列表资源不能绕过远程内容默认阻止。
- 阅读器 iframe 的 sandbox 只有 `allow-popups`，不含 scripts/forms/same-origin/top-navigation。该 token 只让用户点击到达主 WebView 的新窗口回调；宿主始终拒绝应用内窗口创建。
- 远程图片默认由 iframe CSP 阻止，用户可单次显示或启用设备级自动加载；加载使用 `no-referrer`。
- 已收原始 MIME 中被 HTML `cid:` 实际引用、类型为 PNG/JPEG/GIF/WebP/BMP、单项不超过 25MB 且单封累计不超过 100MB 的图片在 Rust 清洗时转换为内存 data URL。CID URL 按 RFC 2392 解码 `%hh` 并与完整 MIME part 集合的规范化 `Content-ID` 匹配，不依赖 `Content-Disposition` 分类；错误声明为 `application/octet-stream` 的 part 只有在解码字节文件魔数可确认上述图片格式时才兼容内联，不信任扩展名。BMP 还必须具有有界的声明文件大小、像素偏移与 DIB 头长度。只有进入最终正文的 part 才从附件区排除，未引用、超限、不支持或文件魔数不匹配的 part 仍作为附件。
- 邮件自身的 `data:image` 只接受通过 media type、编码、文件魔数和共享大小预算验证的 PNG/JPEG/GIF/WebP/BMP；SVG、HTML、未知类型、格式错误和超限内容继续移除。文档 CSP 仍为 `img-src data:`，不会因此获得脚本、同源、本地文件或额外网络权限。
- HTML 清洗版本升级后，正文请求先按账户槽读取本地原始 EML，并在 blocking worker 重新解析、清洗后事务写回；只有原始 EML 缺失或不可解析时才通过 IMAP 重新获取，因此缓存迁移不强制破坏离线重建能力。
- 回复/转发优先从账户隔离的本地原始 EML 解析 HTML part；原文缺失时回退到已缓存的安全 HTML/纯文本，不为打开 Composer 强制联网。compose 专用清洗继续移除脚本、事件、表单、嵌入内容、危险 URL 和 CSS 网络资源，将安全内嵌样式表限定到 `data-nextmail-original-message` 范围。引用原文不再进入 ProseMirror 表格 schema，而以 `sourceHtml` 原子节点保留并在 `sandbox=""`、`no-referrer`、仅 data URL 图片 CSP 的 iframe 中预览；HTML 源码编辑保存时由 Rust 再清洗正文与节点属性。
- 数据格式版本 13 为草稿附件增加 `content_id`/`is_inline`。本地原始 MIME 中实际被 HTML `cid:` 引用的 PNG/JPEG/GIF/WebP/BMP 与用户选择/粘贴图片进入现有 SHA-256 `attachments/` 内容存储；前端只得到不透明 ID、CID 和内存 data URL 预览。图片通过 `src/app/api.ts` 的窄 Command 进入 Rust，验证 MIME、文件魔数、单项 25MB 与总计 100MB 上限。富 HTML 粘贴先由 Rust 保留安全结构、class/ID、行内样式和样式表，再把选择器限定到 `data-nextmail-pasted-html` 容器；未经清洗的剪贴板 HTML 不进入 Composer DOM。远程 `http(s)` 图片不显示占位卡片、不被编辑器静默下载，地址仍随安全 HTML 保存。HTML 源码保真与图片尺寸呈现只调整前端编辑投影，不放宽这里的附件、图片、CSS 或 Rust 清洗边界。
- 签名/模板没有草稿附件归属，插图不伪造无法持久化的 CID。独立定义窗口通过窄 Command 将本地文件交给 Rust，前端先以 3 MiB 限制避免读取超大文件，Rust 再验证 PNG/JPEG/GIF/WebP/BMP 声明、文件魔数和同一大小上限，返回受限 data URL 作为定义 HTML/JSON 的可持久化内容；保存时仍受每种定义内容 5 MB 与 `sanitize_composer_document` 约束。Composer 自身继续使用附件内容存储和 CID，不改为 data URL。
- `http`、`https`、`mailto` 经 Rust 规范化后直接保留为 `href`，固定使用 `_blank` 与 `noopener noreferrer`。相对路径、本机文件、用户信息、反斜线、控制字符、双向文本控制符、危险或未知 scheme 均移除。
- 主窗口 `on_new_window` 对点击目标再次执行同一 Rust 校验，安全目标交给 `state.rs` 注入的系统浏览器/邮件程序打开器，并始终返回 `Deny`；React 没有链接事件、确认 UI 或接受任意 URL 的 IPC。`no-referrer` 保持不变。
- 阅读器不再向邮件文档注入统一字体/行高、16px 内边距、任意断词或图片/表格最大宽度，避免覆盖作者固定宽度和居中布局；阅读页以标题和单一浅边框容器组织基本信息、操作、正文与固定宽度彩色类型附件卡片，iframe 宿主只保留 12px 左右留白。迁移 0011 作为可能已应用的原型保持不可变，0012 删除临时链接表并失效旧缓存；迁移 0014 为受限 `nth-*()` 保真再次失效旧 HTML 缓存，0015 新增本地搜索，0016 增加第十二阶段设置与草稿标记，0021 为标准 CID 与受限 data 图片再次失效旧 HTML 缓存，0022 为阿里云式 octet-stream CID 兼容再次失效已应用缓存，0023 为完整头部验证的 BMP 再次失效安全 HTML 缓存。
- 第十阶段共享语料持续覆盖样式保真和主动内容边界。ADR 0008 已接受并根据第三、四批实机反馈修订；直接外链、传统布局、受限 `nth-*()` flex 表格、Composer 原始 HTML、源码沙箱与 CID 缓存已于 2026-07-21 通过 Windows WebView2 手动验收。ADR 0009 记录 Composer 边界；macOS 未执行。
- 已收附件按账户槽验证归属；高风险扩展名只在文件管理器显示，不自动执行。

## 12. 当前已知技术债与限制

- 当前没有 IMAP IDLE、无 IDLE 轮询或秒级失败重试；新邮件可见性取决于启动、账户周期或手动同步。若未来实现 IDLE，必须作为独立阶段设计长连接重连与现有有界会话预算的共存方式。
- 会话聚合、跨账户搜索、统一收件箱、托盘、系统通知中心、正式签名/公证和自动更新仍未实现或未排期；tag 工作流只提供预览 Release 构建。
- 前端尚无 ESLint/Prettier 和分支/PR CI；当前 GitHub Actions 只响应 `v*` tag push，并执行三平台 bundle 与 Release 发布。
- Vite 主入口压缩后超过 500 kB，写信与设置已动态拆分，但主工作区仍需继续拆包。
- Rust panic、所有稳定 `CommandError` 构造、IMAP/SQLx 原因链、后台 Supervisor/发件 Worker、关键窗口与事件副作用，以及前端未捕获异常均写入同一按日滚动日志。日志观察边界不记录 `CommandError.params`、凭据、Token、邮件正文或服务器原始响应；仍需通过实机故障案例持续校验诊断覆盖率。
- Composer 的 CID/粘贴图片内联发件与阅读器受限 CID 图片展示已实现；远程图片代理/缓存、CSS 背景图和 Web Font 仍未实现。远程图片不会因进入 Composer 而静默下载。

总体阶段顺序见 `plans/master-plan.md`；各阶段范围、进度和验收边界统一记录在 `iterations/`。
