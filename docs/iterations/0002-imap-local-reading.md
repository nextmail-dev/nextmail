# 第二阶段：单账户 IMAP 本地阅读

状态：用户已于 2026-07-12 确认阅读闭环测试通过；受控资源交互差异保留为后续硬化项。

## 实施结果（2026-07-12）

已完成：

- Cargo Workspace、稳定领域/端口边界、SQLx 嵌入式迁移与账户槽隔离。
- 全宽工具栏、单账户静态/多账户下拉显示规则、三栏邮件界面、设置、账户管理、关于和退出。
- 启动后读取本地视图并后台执行只读 IMAP `EXAMINE` 同步；有限重试，不执行 Flags、移动或删除写操作。
- 文件夹、keyset 邮件列表、默认 90 天正文与 30/90/365/全部范围；修改范围后触发后台回填。
- 原始 EML 与附件 SHA-256 内容存储、同步范围外正文和原始邮件按需获取、附件按需解码缓存。
- Rust Ammonia 清洗、严格 CSP 与无脚本 sandbox iframe；远程图片、外链、CID 在未有受控协议前均不会加载或导航。

与原计划的待确认差异：

- “本次加载/始终允许发件人”的远程图片代理、受控外链、CID 资源协议尚未启用；当前采用更严格的全部阻止行为。
- 已下载附件会进入可迁移数据目录并显示完成状态，但通过系统默认程序打开尚未启用。实现它需要引入并验证官方 Tauri opener 或等价的窄权限方案；本次环境未能下载该新依赖，因此没有使用 Shell 绕过。
- 当前同步实现按新 UID 增量抓取并回填缺失正文；完整远端删除与 Flags 对账仍按计划留到第四阶段。

在上述差异得到确认或补齐之前，本阶段不标记为“手动验收通过”。

### 本轮自动验证记录

- `cargo fmt --all -- --check`：通过。
- `cargo test --workspace`：通过，共 22 个 Rust 测试。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `pnpm build`：通过。
- `pnpm test`：通过，共 4 个前端测试。
- `pnpm tauri build --debug --no-bundle`：尚未执行；构建缓存保留。

## 一、阶段目标

在第一阶段已经保存并验证的单个 IMAP 账户基础上，实现一次完整的“服务器读取 → 本地持久化 → 离线重启阅读”闭环：

- 启动后立即展示本地 SQLite 数据，后台再连接服务器更新。
- 同步所有可选择邮件文件夹的结构和邮件摘要。
- 默认预取最近 90 天的文本/HTML 正文；支持 30、90、365 天或全部。
- 正文、原始 EML 和附件按明确状态管理，缺失内容由后台按需获取。
- 提供文件夹列表、邮件列表、纯文本与安全 HTML 阅读界面。
- 保持服务器只读，不在本阶段修改已读、星标、移动、删除或归档状态。

本阶段继续限制为单账户。多账户保存、添加、删除、重新认证和并发 Supervisor 仍属于第五阶段。

## 二、已确认的主界面调整

### 全宽工具栏

主窗口改为两行结构：顶部工具栏横跨整个窗口，下方为账户/文件夹、邮件列表、邮件阅读三栏。

- 移除左上角 NextMail Logo。
- 工具栏左侧显示当前账户：只有一个账户时为静态账户信息；账户数大于一个时才显示下拉箭头和切换菜单。第二阶段实际仍只有一个账户，但组件和测试覆盖多账户显示规则。
- 工具栏保留“收取”和“新建”按钮的正式样式。本阶段按用户确认不绑定行为；使用禁用语义，避免按钮产生无结果的点击。“收取”在完整同步交互阶段接入，“新建”在第三阶段接入。
- 工具栏最右侧使用下拉菜单，包含“设置”“账户管理”“关于 NextMail”“退出”。

### 菜单项目范围

- **设置**：本阶段可用，承接语言、系统/浅色/深色主题和强调色。
- **账户管理**：本阶段可用，显示当前账户连接信息、同步状态和正文同步范围；可修改同步范围。添加/删除账户、修改服务器、更新密码和重新认证不在本阶段显示。
- **关于 NextMail**：本阶段可用，显示产品名、版本和开源许可入口，不显示开发调试信息。
- **退出**：本阶段可用，由窄范围 Rust 命令退出应用，不向前端开放通用进程或 Shell 权限。

## 三、Cargo Workspace 与模块边界

第二阶段开始引入 Cargo Workspace，因为邮件领域、SQLite 和 IMAP 已具备独立依赖和测试边界：

```text
Cargo.toml                 # workspace，仅组织 Rust members
crates/
  nextmail-core/           # 领域模型、Repository/Provider ports、用例、稳定错误
  nextmail-storage/        # SQLx Repository、迁移、raw/attachment/cache 内容存储
  nextmail-protocols/      # IMAP/TLS、MIME 解析、HTML 清洗；未来承载 SMTP/POP3 Adapter
src-tauri/                 # Tauri 命令/事件、窗口、Keyring、本机配置和 Worker 装配
```

依赖方向固定为：`src-tauri` 和 Adapter crates 指向 `nextmail-core`；`nextmail-core` 不依赖 Tauri、SQLx、async-imap、mail-parser 或平台 API。现有首次启动功能先做保持行为不变的机械迁移，通过原有测试后再增加同步功能。

## 四、数据模型与迁移

新增 SQLx 嵌入式迁移，保留现有 `account_slots`，增加以下核心表：

- `mailboxes`：本地 ID、匿名账户槽、远端名称、层级分隔符、属性、标准角色、是否可选择、UIDVALIDITY、UIDNEXT、可选 HIGHESTMODSEQ、总数、未读数、最后同步时间和修订号。
- `messages`：规范化邮件头、主题、发件人/收件人、发送/接收时间、预览、RFC822 大小、Message-ID、References、In-Reply-To、正文可用状态、附件标记和可选原始内容哈希。
- `message_locations`：消息与文件夹的关系、远端 UID、UIDVALIDITY、Flags、可选 MODSEQ 和内部日期；唯一键为文件夹、UIDVALIDITY、UID。
- `message_bodies`：纯文本、清洗后的 HTML、正文下载状态和内容修订号。
- `attachments`：MIME part 路径、文件名、类型、大小、Content-ID、disposition、下载状态和可选内容哈希。
- `sync_states`：账户/文件夹同步阶段、最后成功时间、最后 UID、重试次数和稳定错误码。
- `remote_image_permissions`：按账户与规范化发件人保存“始终允许”选择；不保存远程图片 URL 明文作为偏好索引。

地址列表和 Flags 首期以版本化 JSON 列保存，但 Repository 对外返回强类型；数据库 JSON 结构不作为前端 API。所有列表使用稳定排序和 keyset cursor，不使用大 offset 分页。

### 去重规则

- 远端位置始终以 `(mailbox, uid_validity, uid)` 唯一识别。
- 有服务商稳定消息 ID 时优先使用服务商 ID；否则仅在 Message-ID、RFC822 大小和关键日期同时一致时合并规范化消息。
- Message-ID 缺失或存在冲突时宁可保留独立消息，避免误合并。

## 五、文件存储

- `raw/<aa>/<bb>/<sha256>.eml`：完整原始邮件，临时文件写完并校验哈希后原子改名。
- `attachments/<aa>/<bb>/<sha256>`：解码后的附件内容，文件名只作为数据库元数据，不参与路径拼接。
- `cache/`：可重建的远程图片和阅读缓存。

数据库事务只提交已经成功落盘的内容哈希；失败或取消时清理临时文件。前端只接收不透明资源 ID，不接收真实数据目录路径。

## 六、IMAP 同步策略

### 连接与文件夹发现

- 复用第一阶段已经验证的 TLS、STARTTLS 和无加密连接策略，从系统凭据库取密码。
- 登录后读取 Capability，使用 `LIST "" "*"` 获取所有文件夹、层级分隔符和属性。
- `\Noselect` 文件夹只保留层级节点，不执行邮件同步。
- 根据 SPECIAL-USE 属性识别 Inbox、Sent、Drafts、Trash、Junk、Archive；无法识别时保留普通文件夹，不猜测写操作目标。

### 首次同步

每个可选择文件夹串行执行，单账户同时最多一个远端同步会话：

1. 使用只读 `EXAMINE`；服务器支持 CONDSTORE 时使用只读能力获取 MODSEQ，但本阶段不依赖 QRESYNC。
2. 保存 UIDVALIDITY/UIDNEXT；使用 UID SEARCH 获取远端 UID 集合。
3. 按每批 100 个 UID 获取 UID、FLAGS、INTERNALDATE、RFC822.SIZE、ENVELOPE、需要的会话头和 BODYSTRUCTURE。
4. 每批写入一个短事务并提高文件夹修订号，避免整个账户长事务锁库。
5. 摘要完成后，后台低优先级预取同步范围内的纯文本/HTML MIME part；附件 part 不预取。

正文获取一律使用 `BODY.PEEK[...]`，不会因为阅读或同步隐式设置服务器 `\Seen`。`async-imap` 提供 UID FETCH、BODYSTRUCTURE、section 和只读 EXAMINE 能力，Adapter 内部负责把协议类型映射为领域 DTO。

### 后续启动

- UIDVALIDITY 未变化时只抓取高于最后 UID 的新摘要，并补齐未完成正文任务。
- UIDVALIDITY 变化时废弃该文件夹旧远端位置映射并重新建立，规范化消息和已下载内容在确认无引用后延迟清理。
- 本阶段不持续运行 IMAP IDLE，不周期轮询，不完整同步远端 Flags 和删除；这些属于第四阶段。
- 网络失败使用有限指数退避，退出应用时可取消；重启后从数据库状态继续，不清空已有本地邮件。

## 七、MIME、正文与附件

- 摘要阶段解析必要邮件头和 BODYSTRUCTURE，保存正文与附件 part 路径。
- 正文任务只下载 text/plain、text/html 及正文所需的内联资源，按 Content-Transfer-Encoding 和字符集解码。
- 需要完整恢复、查看原始内容时再请求 `BODY.PEEK[]`，使用 `mail-parser` 解析并以 SHA-256 保存原始 EML。
- 点击附件后才下载对应 MIME section；下载成功后可由 Rust 交给系统默认程序打开，前端不获得路径。
- 正文获取优先级高于 90 天后台预取，用户当前打开的邮件优先于同步队列。

## 八、HTML 邮件安全

Rust 使用 Ammonia 的白名单 Builder 作为第一层，并增加邮件专用处理：

- 完全移除 script、form、iframe、object/embed、SVG/MathML、事件属性、meta refresh、危险 URL 和外部样式表。
- 内联 CSS 只允许审查过的排版、颜色、表格和间距属性；移除 `url()`、position、z-index、behavior 和可能突破阅读区域的规则。
- `cid:` 和已下载本地资源改写为 `nextmail-resource://<opaque-id>`。
- http/https/mailto 链接改写为 `nextmail-link://<opaque-id>`；Rust 保留真实目标并在触发时再次校验协议。
- 远程图片默认不请求。用户可选择“本次加载”或“始终允许该发件人”；远程请求由 Rust 代理，限制协议、重定向、私网地址、响应大小和 MIME 类型。

清洗结果放入专用 `SafeMailFrame`：iframe 不包含 `allow-scripts`、`allow-forms`、`allow-same-origin` 或 top-navigation；frame 文档 CSP 为 `default-src 'none'`，只允许安全内联样式、data:image 和受控本地资源协议。HTML 不直接插入主 React DOM。

安全清洗和资源协议均使用恶意邮件语料单元测试，覆盖事件属性、混淆 javascript URL、SVG、CSS URL、表单、远程跟踪像素、路径穿越和超大响应。

## 九、Rust Worker、命令与事件

### Worker

`AccountSupervisor` 在应用 ready 后启动单账户任务，职责包括首次/恢复同步、用户请求正文/附件/原始 EML、任务优先级、取消、退避和状态持久化。数据库 Pool 在应用生命周期内复用，不再每次命令打开后立即关闭。

### 公共类型

- `MailboxRole`、`MailboxSummary`
- `MessageListItem`、`MessageListPage`、`MessageDetail`
- `BodyAvailability`、`AttachmentSummary`、`AttachmentAvailability`
- `SyncPhase`、`SyncProgress`、`AccountSyncSettings`
- `RemoteImagePolicy`、`ContentRevision`

### 查询和请求命令

- `list_mailboxes(account_id)`
- `list_messages(account_id, mailbox_id, cursor, limit)`
- `get_message_detail(account_id, message_id)`
- `get_sync_progress(account_id)`
- `request_message_body(account_id, message_id)`
- `request_attachment(account_id, attachment_id)`
- `request_raw_message(account_id, message_id)`
- `set_remote_image_permission(account_id, message_id, decision)`
- `get_account_management_detail(account_id)`
- `set_account_sync_settings(account_id, settings)`
- `get_app_about()`
- `quit_app()`

读取命令只访问 Repository；需要联网的操作只向 Worker 入队并立即返回任务状态。错误继续使用稳定 `CommandResult<T>`，不返回密码、内部路径或原始服务器响应。

### 事件

- `sync-progress`：账户 ID、阶段、已完成/总数、修订号。
- `mailbox-changed`：账户 ID、文件夹 ID、修订号。
- `message-content-changed`：账户 ID、消息 ID、修订号。
- `sync-failed`：账户 ID、稳定错误码、是否可重试。

事件不携带邮件正文、附件字节或服务器原始响应。前端收到事件后只失效对应 TanStack Query。

## 十、前端功能

- `mail-shell`：全宽工具栏、账户展示规则、三栏自适应布局和右侧菜单。
- `mailboxes`：文件夹树、标准角色图标、未读数、同步/错误状态。
- `message-list`：每页 50 条 keyset 无限查询、稳定滚动、发件人、主题、预览、日期和附件标记。
- `message-viewer`：头部、收件人展开、正文状态、纯文本/安全 HTML 切换、远程图片提示和附件列表。
- `settings`：语言、主题和强调色，继续使用现有偏好命令。
- `accounts`：当前账户只读连接信息、同步范围和同步状态。
- `about`：产品版本和开源许可入口。

启动时主界面不会等待网络：先显示本地文件夹和邮件，Worker 状态以非阻塞方式呈现。没有本地数据时显示正式同步空状态和进度；断网时继续浏览已有内容。

## 十一、Capability 与权限

- 主窗口继续不获得任意文件、数据库、Shell 或通用网络访问。
- 退出、受控链接、附件打开和邮件资源读取均由自有窄命令/协议完成。
- 如果邮件资源使用独立 WebView/协议权限，单独建立 capability，避免与主窗口权限合并。
- CSP 不增加任意远程图片或页面来源；开发地址仅保留在开发构建需要的配置中。

## 十二、明确不在本阶段的内容

- 多账户添加、删除、重新认证和真实账户切换。
- 工具栏“收取”“新建”的点击行为。
- IMAP IDLE、周期轮询、远端 Flags 完整同步。
- 已读、星标、移动、复制、删除、归档和离线操作队列。
- 写信、草稿、SMTP 发件、签名和模板。
- FTS 搜索和会话聚合。
- POP3、OAuth、托盘和通知。

## 十三、实施顺序

1. 建立 Cargo Workspace，机械迁移现有代码并保持第一阶段测试全部通过。
2. 重构全宽工具栏和三栏 AppShell，完成菜单、设置、账户管理、关于和退出；动作按钮暂不接行为。
3. 增加 SQLite 迁移、Repository、内容寻址文件存储和恢复测试。
4. 实现 IMAP 只读 Adapter、文件夹/摘要同步和持久化同步状态。
5. 实现 Supervisor、任务优先级、事件和失败恢复。
6. 实现文件夹、邮件列表和离线查询界面。
7. 实现 MIME part 正文、Ammonia 清洗、sandbox viewer、受控资源/链接协议和远程图片策略。
8. 实现附件与原始 EML 按需下载、缓存和打开。
9. 完成自动验证、文档更新和 Windows 手动验收，等待用户确认后再规划第三阶段。

## 十四、自动验证

- Rust：核心用例、SQLite 临时库迁移/Repository、内容哈希、IMAP transcript、MIME、HTML 安全语料、Worker 恢复测试。
- 前端：单/多账户工具栏条件、菜单键盘操作、文件夹和邮件列表查询、正文状态、sandbox 属性、事件失效查询测试。
- 检查：`cargo fmt --all -- --check`、`cargo test --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`pnpm test`、`pnpm build`、`pnpm tauri build --debug --no-bundle`。
- 构建缓存 `dist` 和 `src-tauri/target` 默认保留；清理临时数据库、协议服务、测试凭据、日志、coverage 和安全测试输出。

## 十五、手动验收

1. 主界面无左上角 Logo，工具栏横跨窗口；单账户无下拉箭头，模拟多账户组件时可键盘切换。
2. “收取”“新建”呈现正式但禁用的工具栏样式；右侧菜单、设置、账户管理、关于和退出行为正确。
3. 全新本地库启动后可同步所有可选择文件夹、摘要和默认 90 天正文，过程中界面可操作。
4. 退出并断网重启后，文件夹、邮件列表和已经下载的正文可立即阅读。
5. 重复启动不会产生重复邮件；UIDVALIDITY 变化可安全重建远端位置。
6. 打开邮件不会把服务器邮件标记为已读；本阶段不执行任何移动、删除或 Flags 写入。
7. HTML 邮件无法执行脚本、表单、导航或任意本地访问；远程图片默认不请求。
8. 本次/发件人放行远程图片、受控外链、CID 图片和附件按需下载行为符合提示。
9. 数据目录中包含邮件内容但不包含账户密码、Token 或服务器账户配置；前端看不到真实文件路径。
10. Windows 10 22H2+ 使用真实 IMAP 账户完成验收；macOS 仅在实际执行后记录结果。

## 十六、参考依据

- [async-imap Session 0.11.2](https://docs.rs/async-imap/latest/async_imap/struct.Session.html)
- [async-imap Fetch/BODYSTRUCTURE](https://docs.rs/async-imap/latest/async_imap/types/struct.Fetch.html)
- [mail-parser MessageParser](https://docs.rs/mail-parser/latest/mail_parser/struct.MessageParser.html)
- [Ammonia whitelist Builder](https://docs.rs/ammonia/latest/ammonia/struct.Builder.html)
- [Tauri Capability](https://v2.tauri.app/security/capabilities/)
- [Tauri 前后端命令与事件](https://v2.tauri.app/develop/calling-rust/)
