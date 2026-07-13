# NextMail 架构基线

## 运行边界

NextMail 使用单个 Tauri 进程。React 仅通过稳定的 Tauri Command DTO 读取本地视图或提交业务命令，不直接连接 SQLite、邮件服务器、文件系统或系统凭据库。

Rust 代码使用 Cargo Workspace 分为：

- `crates/nextmail-core`：不依赖 Tauri、数据库和协议库的领域 DTO、稳定错误与 ports。
- `crates/nextmail-storage`：SQLx Repository、嵌入式迁移和内容寻址文件存储。
- `crates/nextmail-protocols`：IMAP 同步与写操作、MIME 解析/生成和 HTML 清洗；后续继续承载 POP3 Adapter。
- `src-tauri`：首次启动用例、Keyring、自动发现、Command/Event 和 Worker 装配。

协议库类型不得越过 Adapter。命令错误只返回稳定错误码、可本地化参数和是否可重试，不返回密码、服务器原始响应或内部堆栈。

### Rust 包拆分策略

第二阶段已经引入 Cargo Workspace。包边界服务于独立依赖与测试隔离，不按每个页面或小功能创建 crate：

- `crates/nextmail-core`：纯 Rust 的领域模型、用例接口和稳定错误。
- `crates/nextmail-protocols`：IMAP、POP3、SMTP 与 MIME Adapter。
- `crates/nextmail-storage`：SQLite、原始邮件、附件和索引存储。
- `src-tauri`：窗口、Capability、系统集成和 Command/Event 薄壳。

依赖方向保持为宿主和 Adapter 指向核心，核心不得依赖 Tauri、SQLx 或具体协议库。

## 存储边界

用户选择的数据目录是可迁移数据集，当前包含：

- `.nextmail-data.json`：格式版本和匿名数据集 ID。
- `content.sqlite`：匿名账户槽、文件夹、邮件、远端位置、正文、草稿、附件元数据、发件任务、待办操作与同步状态。
- `raw/`：按 SHA-256 分层保存的收取和待发送原始 EML。
- `attachments/`：按 SHA-256 分层、去重保存的已下载附件和草稿附件副本。
- `cache/`：可重建缓存的保留目录。

邮箱地址、服务器配置、数据槽映射、首次启动状态和外观设置位于 Tauri 系统应用配置目录。密码只以 `com.taurusxin.nextmail` 服务项写入 Windows Credential Manager 或 macOS Keychain。

数据目录初始化只接受空目录或兼容的 NextMail 目录。新建过程失败时仅清理本次创建的标记、数据库和空子目录，不递归删除用户原有内容。

## 连接安全

- 全进程统一使用 rustls `ring` CryptoProvider，并在 Tauri 初始化前显式安装；直接 TLS 依赖关闭默认 provider 特性，避免依赖合并后出现 provider 歧义。
- IMAP 支持无加密、STARTTLS 和隐式 TLS；TLS 使用系统根证书并严格校验主机名。
- SMTP 使用 lettre、Tokio 和 rustls；连接测试只认证账户，正式发件使用持久化 MIME 和 `send_raw`。
- 无加密连接必须由用户显式确认，后端在保存时再次校验该确认。
- 自动发现顺序为内置服务商、DNS SRV、域名 HTTPS autoconfig。自动配置响应限制为 1 MiB 且不接受 HTTP 降级。
- 账户保存顺序为连接验证、匿名数据槽、系统凭据、外置账户配置；任一步失败都会补偿此前写入。

## IMAP 同步与离线操作

- 单账户 `AccountSupervisor` 常驻 Rust 端。启动先返回 SQLite 本地视图，再执行网络同步；Inbox 使用独立 IDLE 会话，服务器不支持时回退轮询。
- 同步按 UIDVALIDITY/UID 定位邮件，拉取新 UID，并对账当前 UID 集合、Flags 和 MODSEQ。UIDVALIDITY 改变时重建文件夹位置，不使用消息序号作为持久身份。
- 用户修改在 SQLite 事务中同时更新本地投影和 `pending_operations`。Worker 按顺序执行，`running` 状态可在重启后恢复。
- Flags 以增量意图写回；CONDSTORE 可用时使用条件 STORE 并在冲突后基于最新 MODSEQ 重放一次。
- MOVE、UIDPLUS、CONDSTORE 和 IDLE 全部在自有 Adapter 内做 Capability 分支。缺失 UIDPLUS 时不执行可能影响其他邮件的宽泛 EXPUNGE。
- React 事件只收到账户、文件夹、消息或操作 ID 与修订状态，并通过 TanStack Query 重新读取本地视图。
- Supervisor 区分“仅执行待办”和“执行同步”：本地 Flags、移动、复制、删除及 APPEND 只唤醒待办 Worker，不再先完整同步。首次启动和手动收取发布可见进度；IDLE/定时/轮询同步静默落库并只通知数据变化。
- 支持 IDLE 时由服务器变化即时唤醒，并以 5 分钟全文件夹对账兜底；无 IDLE 时每 60 秒轮询。网络错误退避范围为 2 秒到 5 分钟。

## 草稿与发件边界

- 独立 `composer-*` WebView 通过窄业务命令访问草稿，不直接访问数据库、任意文件或网络；系统文件选择器只授权用户明确选择的附件。
- 草稿保存 Tiptap JSON、HTML 和纯文本，使用修订号做乐观并发控制。写信窗口关闭前会提交未保存改动。
- SMTP 联网前先用 `mail-builder` 生成完整 UTF-8 MIME，按内容哈希原子落盘并创建 `send_job`。Bcc 只进入 SMTP envelope，不写入邮件头。
- 后台 `SendWorker` 从系统凭据库取密码，串行发送不可变 MIME；临时错误最多自动尝试三次，失败内容继续保留并支持显式重试。
- 异常退出遗留的 `sending` 在启动时恢复为 `queued`。SMTP 成功后独立排队 APPEND 到映射的 Sent；Sent 归档失败不会触发再次 SMTP 发送。
- 本地草稿停止编辑 10 秒或关闭窗口时排队同步到映射的 Drafts。远端版本用 `X-NextMail-Draft-ID` 关联，先追加新版本再安全清理旧 UID；服务器草稿可转换成本地可编辑草稿。
- Tiptap 写信代码按窗口动态加载，不进入主窗口首包。
- 完全未修改的空草稿在写信窗口关闭时由 Rust 条件删除；前端不能直接删除任意草稿。SMTP 成功通过 ID/状态事件通知主窗口，由主窗口显示站内成功通知。
- 回复、回复全部和转发由 Rust 从本地规范邮件生成新草稿。回复草稿保存 `In-Reply-To`/`References` 并在 MIME 生成时安全注入；回复全部排除自身并去重，转发按需取得原附件后复用内容寻址副本。

## 邮件与文件夹编码

- RFC 2047 邮件头、结构化地址、MIME 正文和附件名统一由启用 `full_encoding` 的 `mail-parser` 解码；NextMail 不维护第二套 encoded-word 或字符集解析器，只保留领域 DTO 映射与回归语料。
- 支持 GB2312/GBK/GB18030、Big5、Shift-JIS、EUC-KR、Windows code pages、ISO-8859 系列和 Unicode 编码；未知或畸形 RFC 2047 encoded-word 保留原文并继续解析后续字段，不用系统区域设置猜测。
- IMAP 远端文件夹名保留线缆原值用于 `EXAMINE`，另生成 modified UTF-7 解码后的 Unicode 显示名，避免显示名反向影响协议定位。
- 标准文件夹由 `MailboxRole` 本地化，用户创建的其他文件夹保留服务端名称语义。

## 前端设计系统

前端采用 shadcn 的源码归属模式而不是安装黑盒组件库：组件源码位于 `src/components/ui/`，每个组件独立文件，可按产品需求修改。Radix 只提供无样式的键盘、焦点和 ARIA 行为。

主题使用 shadcn 语义 CSS Variables，并通过 Tailwind v4 映射为工具类。视觉基线结合 Nova 的紧凑密度和 Lyra 的利落几何：基础圆角为 4px，控件使用矩形或微圆角，不采用 shadcn 默认外观。普通导航项和邮件行以背景、留白和文字层级表达状态，不为每个元素绘制边框；边框只承担侧栏、工具栏和内容栏的结构性分隔。深色主题的表面令牌为无色度灰黑色，强调色仍可独立切换。

主窗口当前采用“左侧账户与文件夹、右侧工具栏、邮件列表和正文”的层级。账户切换属于侧栏顶部；新建与本地草稿组成文件夹上方的主操作组合按钮；收取、回复、回复全部、转发、复制、当前文件夹即时过滤和应用菜单属于顶部工具栏。文件夹栏和邮件列表可在最小/最大宽度内拖动，文件夹栏可折叠为保留可访问名称的图标模式。当前搜索只过滤已加载邮件，不等同于后续 FTS 全文搜索。邮件列表采用连续行与底部分隔线，不使用独立卡片间距。

业务页面消费拆分后的布局、文本、表单、选择器、提示和空状态组件，原则上不直接使用原生表单控件。中文与英文文案由独立 JSON 语言包提供，不在功能组件中写死生产文案。首次设置保留语言切换；进入主界面后，语言、主题和强调色统一由工具栏菜单中的“设置”承载。

## HTML 阅读器

- 清洗层移除脚本、表单、嵌入文档、事件属性、危险 URL 和 CSS 资源；安全的远程图片 URL 可以保留在清洗结果中，但默认 iframe CSP 的 `img-src data:` 阻止请求。
- “立即显示”只把当前 iframe 的图片 CSP 扩为 `data: http: https:`，sandbox 仍不启用 scripts、forms、same-origin 或 top-navigation，并使用 `no-referrer`。
- Tauri 顶层 CSP 允许图片协议只是为 iframe 的显式选择提供上限；默认阻止由邮件文档自身更严格的 CSP 执行。
- 阅读 iframe 不继承应用 DOM 样式。NextMail 根据有效主题注入浅色或灰黑深色阅读层；深色层覆盖常见文字与背景，保证旧邮件中硬编码颜色的基本可读性。
