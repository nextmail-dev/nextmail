# NextMail 架构基线

## 运行边界

NextMail 使用单个 Tauri 进程。React 仅通过稳定的 Tauri Command DTO 读取本地视图或提交业务命令，不直接连接 SQLite、邮件服务器、文件系统或系统凭据库。

主窗口启动遵循“首帧和本地视图优先”顺序：Tauri `setup` 只装配状态，不启动邮件同步或发件 Worker；React 首先显示随 HTML 内嵌的中性加载层，再读取 Bootstrap、外观配置和 SQLite 本地视图。主工作区完成至少一个绘制周期后，通过幂等业务命令启动后台服务。同步不会作为进入主界面的前置条件，文件夹完成事件持续失效对应查询，使新文件夹、邮件摘要和正文随着 SQLite 落库逐步出现。

Rust 代码只使用 `src-tauri` 下的单一 Cargo package，避免仓库根目录和 Tauri 目录各自产生一套 `Cargo.lock` 与 `target`：

- `src-tauri/src/core`：不依赖 Tauri、数据库和具体协议库的领域 DTO、稳定错误与 ports。
- `src-tauri/src/storage`：共享 SQLite/内容存储之上的读取、同步写入、模板/签名、草稿、发件任务、待办操作和文件夹角色子仓库；嵌入式迁移位于 `src-tauri/migrations`。
- `src-tauri/src/protocols`：IMAP 同步与写操作、MIME 解析/生成和 HTML 清洗；IMAP 内部再按会话、解析、文件夹编码和同步策略拆分。新增协议 Adapter 只有在未来单独排期后进入此边界。
- `src-tauri/src/application`、`adapters`、`commands` 与运行时模块：首次启动用例、Keyring、自动发现、Command/Event、窗口和 Worker 装配。

仓库根目录不放置 Cargo manifest、lockfile 或 Rust 构建目录。唯一的 `Cargo.toml`、`Cargo.lock` 和 `target` 均由 `src-tauri` 管理。

协议库类型不得越过 Adapter。命令错误只返回稳定错误码、可本地化参数和是否可重试，不返回密码、服务器原始响应或内部堆栈。

### 窗口与 Capability

- Windows 主窗口和动态创建的写信/设置窗口关闭系统 decorations，由 React 标题栏提供拖动、最小化、最大化和关闭按钮。
- macOS 保留系统 decorations，使用 `Overlay` 标题栏和原生交通灯；React 只提供可拖动内容区，不伪造 macOS 窗口按钮。
- 自绘标题栏使用紧凑高度；Windows 窗口按钮和 macOS 交通灯只保留满足拖动与原生操作所需的最小安全空间。站内通知通过根节点 Portal 渲染在标题栏下方，避免被工作区裁剪或窗口拖动层遮挡。
- `main`、`composer-*`、`settings` 继续使用独立 Capability。窗口控制只开放启动拖动、最小化、切换最大化和关闭；写信窗口因发送成功需要绕过关闭拦截，额外保留 `allow-destroy`。
- 设置使用单例 `settings` WebView。重复打开只聚焦已有窗口；偏好变化由 Rust 持久化后发布窄事件，各窗口把 DTO 写入各自的 TanStack Query cache 并更新主题和语言，不共享 React 内存状态。外观写入使用乐观 cache 更新，失败时恢复旧值。

### Rust 模块拆分策略

NextMail 不再用多个 Cargo package 表达业务边界，而是在单一 `src-tauri` package 内保持清晰模块：

- `core`：纯 Rust 的领域模型、用例接口和稳定错误。
- `protocols`：当前 IMAP、SMTP、MIME 和 HTML 安全 Adapter。
- `storage`：SQLite、原始邮件、附件和索引存储。
- Tauri 宿主模块：窗口、Capability、系统集成、Command/Event 和运行时装配。

依赖方向仍保持为宿主和 Adapter 指向核心，核心不得依赖 Tauri、SQLx 或具体协议库。协议库与 SQLx 类型不得越过模块边界；模块级单元测试、公共 DTO 审查和受控可见性用于维持原有隔离。除非未来出现独立发布、独立版本或被其他二进制复用的实际需求，不再为形式上的分层创建 Cargo Workspace 或子 crate。

账户、Bootstrap 与本机偏好的配置读写以 `core::ports` 注入 application service；IMAP Provider、Repository Provider、系统附件与外链打开能力同样由 `state.rs` 装配。Application 不构造具体 JSON Store，Worker 不构造具体 IMAP/SQLite Adapter。写信与邮件运行时复用同一个 Repository 实例和 SQLite 连接池。模板与签名输入校验、变量渲染及初始三格式组合位于 application，SQLx Repository 只按显式作用域持久化定义、场景引用和 revision。

## 存储边界

用户选择的数据目录是可迁移数据集，当前包含：

- `.nextmail-data.json`：格式版本和匿名数据集 ID。
- `content.sqlite`：匿名账户槽、文件夹、邮件、远端位置、正文、模板、签名、草稿、附件元数据、发件任务、待办操作与同步状态。
- `raw/`：按 SHA-256 分层保存的收取和待发送原始 EML。
- `attachments/`：按 SHA-256 分层、去重保存的已下载附件和草稿附件副本。
- `cache/`：可重建缓存的保留目录。

已下载邮件附件仍以无扩展名的内容哈希保存在 `attachments/`。用户打开附件时，Rust 在 `cache/attachment-open/` 下按不透明附件 ID 与内容哈希生成带安全文件名的可重建副本；原始数据路径和缓存路径都不返回 React。

邮箱地址、服务器配置、数据槽映射、首次启动状态、外观设置和阅读偏好位于 Tauri 系统应用配置目录。`accounts.json` 使用单调修订号、进程内串行变更锁和原子文件替换维护多账户集合；最近选择账户与不含邮箱或秘密的待清理凭据引用也保存在此。阅读偏好独立写入 `reading-preferences.json`，不会随邮件数据目录迁移；密码只以 `com.taurusxin.nextmail` 服务项写入 Windows Credential Manager 或 macOS Keychain。

数据目录初始化只接受空目录或兼容的 NextMail 目录。新建过程失败时仅清理本次创建的标记、数据库和空子目录，不递归删除用户原有内容。

## 连接安全

- 全进程统一使用 rustls `ring` CryptoProvider，并在 Tauri 初始化前显式安装；直接 TLS 依赖关闭默认 provider 特性，避免依赖合并后出现 provider 歧义。
- IMAP 支持无加密、STARTTLS 和隐式 TLS；TLS 使用系统根证书并严格校验主机名。IMAP 同步与首次账户连接测试共享进程级 rustls 配置，系统根证书只在首次 TLS 连接时加载一次。
- SMTP 使用 lettre、Tokio 和 rustls；连接测试只认证账户，正式发件使用持久化 MIME 和 `send_raw`。
- 无加密连接必须由用户显式确认，后端在保存时再次校验该确认。
- 自动发现顺序为内置服务商、DNS SRV、域名 HTTPS autoconfig。自动配置响应限制为 1 MiB 且不接受 HTTP 降级。
- 新增账户按连接验证、匿名数据槽、系统凭据、外置账户配置的顺序提交；任一步失败都会补偿此前写入。编辑密码先写新凭据引用，再把新配置和旧引用清理任务原子提交，最后幂等清理旧凭据。移除账户同样先把配置移除与清理任务一起提交，凭据库临时失败不会恢复已移除账户或留下明文秘密。

## IMAP 同步与离线操作

- `MailRuntime` 作为 Supervisor Registry，按 `account_id` 维护至多一个 `AccountSupervisor`。每个账户独立拥有唤醒、启动同步、退避、待办重放和 Inbox IDLE 生命周期；所有账户共享一个 Repository/SQLite 连接池，并始终通过匿名 `account_slot_id` 隔离数据。
- 主动 IMAP 同步和写操作共享两个全局许可，同一账户由自身 Supervisor 保持串行；IDLE 等待不占主动网络许可。新增、编辑、重新认证和移除账户均在运行期协调，旧代次任务返回的状态和界面事件会被丢弃。
- Supervisor 只在主工作区完成首帧后启动；启动同步在内存中预先进入 `connecting` 状态，进度查询即使错过最早事件也能读到当前阶段。运行时启动和发件 Worker 启动均为幂等操作，可安全承受 React Strict Mode 或窗口状态变化导致的重复通知。
- 同步按 UIDVALIDITY/UID 定位邮件，拉取新 UID，并对账当前 UID 集合、Flags 和 MODSEQ。新邮件正文与旧正文回填均按最多 100 个 UID 批量 FETCH；文件夹对账通过事务内远端 UID 集合做集合删除，不按本地位置逐行查询。UIDVALIDITY 改变时重建文件夹位置，不使用消息序号作为持久身份。
- 用户修改在 SQLite 事务中同时更新本地投影和 `pending_operations`。Worker 按顺序执行，`running` 状态可在重启后恢复。
- Flags 以增量意图写回；CONDSTORE 可用时使用条件 STORE 并在冲突后基于最新 MODSEQ 重放一次。
- MOVE、UIDPLUS、CONDSTORE 和 IDLE 全部在自有 Adapter 内做 Capability 分支。缺失 UIDPLUS 时不执行可能影响其他邮件的宽泛 EXPUNGE。
- React 事件只收到账户、文件夹、消息或操作 ID 与修订状态，并通过 TanStack Query 重新读取本地视图。邮件详情 key 统一为账户、文件夹、消息四段；缺少文件夹 ID 的正文事件按账户前缀失效，避免把消息 ID 放入错误槽位。
- 网络读取仍在异步 IMAP 会话中完成；MIME 解码、正文预览、附件分析和 HTML 清洗在顺序提交的 Tokio blocking worker 中执行，避免大邮件解析占用异步调度线程。文件夹元数据落库后立即发布 `mailbox-changed`，其后每批最多 100 封摘要落库再次发布，既不等待整个文件夹，也不等待整个账户同步结束。
- 单封邮件的规范记录、远端位置、正文和附件元数据在同一 SQLite 事务内提交，附件元数据使用批量 UPSERT。内容寻址原始 EML 在事务前完成幂等文件写入，数据库失败不会留下可见的半成品邮件。
- Supervisor 区分“仅执行待办”和“执行同步”：本地 Flags、移动、复制、删除及 APPEND 只唤醒待办 Worker，不再先完整同步。首次启动和手动收取发布可见进度；IDLE/定时/轮询同步静默落库并只通知数据变化。
- 支持 IDLE 时由服务器变化即时唤醒，并以 5 分钟全文件夹对账兜底；无 IDLE 时每 60 秒轮询。网络错误退避范围为 2 秒到 5 分钟。

## 草稿与发件边界

- 模板与签名使用独立窄 Repository 保存三格式富文本定义和四场景规则。全局定义/规则使用空账户槽，账户记录始终通过 Rust 将公开账户 ID 解析为匿名 `account_slot_id`；React 不接触数据槽。账户场景没有显式记录时整体继承全局规则，显式账户规则优先；引用范围在 Repository 边界验证，被引用定义禁止删除。
- 变量白名单、缺失上下文错误、HTML/主题/纯文本差异化转义与本地化日期在 application 完成。设置窗口和 Composer 只通过 `src/app/api.ts` 使用稳定 DTO；Composer 先取得可见定义摘要，再把当前收件人上下文交给 Rust 渲染。
- Tiptap 使用 `nextmailTemplate` 和 `nextmailSignature` 可编辑块节点保存定义 ID，HTML 使用对应 `data-nextmail-*-id` 属性。显式切换只替换同类节点，自动规则只在草稿首次创建时应用，因此用户手动删除签名后自动保存或重开不会恢复。
- 独立 `composer-*` WebView 通过窄业务命令访问草稿，不直接访问数据库、任意文件或网络；系统文件选择器只授权用户明确选择的附件。
- 草稿保存 Tiptap JSON、HTML 和纯文本，使用修订号做乐观并发控制。写信窗口关闭前会提交未保存改动；关闭监听按账户与草稿身份单次订阅，并通过 ref 读取最新保存函数和编辑状态。
- SMTP 联网前先用 `mail-builder` 生成完整 UTF-8 MIME，按内容哈希原子落盘并创建 `send_job`。MIME `Date` 头在生成时读取操作系统本机时区并写入当时的 UTC 偏移；Bcc 只进入 SMTP envelope，不写入邮件头。
- 后台 `SendWorker` 从系统凭据库取密码，按账户内 FIFO、账户间轮转方式发送不可变 MIME；全局最多同时发送两封，同一账户同时最多一封。单个账户的超时、断网或认证错误不会阻塞其他账户；临时错误最多自动尝试三次，失败内容继续保留并支持显式重试。
- 异常退出遗留的 `sending` 在启动时恢复为 `queued`。SMTP 成功后独立排队 APPEND 到映射的 Sent；Sent 归档失败不会触发再次 SMTP 发送。
- 本地草稿停止编辑 10 秒或关闭窗口时排队同步到映射的 Drafts。远端版本用 `X-NextMail-Draft-ID` 关联，先追加新版本再安全清理旧 UID；服务器草稿可转换成本地可编辑草稿。
- Tiptap 写信代码按窗口动态加载，不进入主窗口首包。
- 完全未修改的空草稿在写信窗口关闭时由 Rust 条件删除；前端不能直接删除任意草稿。SMTP 成功通过 ID/状态事件通知主窗口，由主窗口显示站内成功通知。
- 回复、回复全部和转发由 application 层的纯用例从本地规范邮件生成新草稿，Repository 只读取源邮件并持久化组合结果。回复草稿保存 `In-Reply-To`/`References` 并在 MIME 生成时安全注入；回复全部排除自身并去重，转发按需取得原附件后复用内容寻址副本。

## 邮件与文件夹编码

- RFC 2047 邮件头、结构化地址、MIME 正文和附件名统一由启用 `full_encoding` 的 `mail-parser` 解码；NextMail 不维护第二套 encoded-word 或字符集解析器，只保留领域 DTO 映射与回归语料。
- 支持 GB2312/GBK/GB18030、Big5、Shift-JIS、EUC-KR、Windows code pages、ISO-8859 系列和 Unicode 编码；未知或畸形 RFC 2047 encoded-word 保留原文并继续解析后续字段，不用系统区域设置猜测。
- IMAP 远端文件夹名保留线缆原值用于 `EXAMINE`，另生成 modified UTF-7 解码后的 Unicode 显示名，避免显示名反向影响协议定位。服务端返回的层级分隔符随文件夹 DTO 传给界面；文件夹树只按该分隔符连接已存在的父节点，不猜测名称中的 `/`、`.` 等字符。
- 标准文件夹由 `MailboxRole` 本地化，用户创建的其他文件夹保留服务端名称语义。

## 前端设计系统

前端采用 shadcn 的源码归属模式而不是安装黑盒组件库：组件源码位于 `src/components/ui/`，每个组件独立文件，可按产品需求修改。Radix 只提供无样式的键盘、焦点和 ARIA 行为。

主题使用 shadcn 语义 CSS Variables，并通过 Tailwind v4 映射为工具类。当前视觉基线为现代 SaaS 风格：浅色主题使用清新的白色与中性灰表面，深色主题使用无色度灰黑表面；用户可见的“主题色”在内部作为强调色令牌，独立派生选中背景、焦点环和主操作。控件以背景、留白、阴影和文字层级表达状态，普通按钮、输入框、弹层、导航项和内容区域不绘制装饰性边框。基础圆角为 10px，保留清晰几何而不使用拟物效果。

UI 使用操作系统原生字体栈，不再随 Vite 打包字体。Windows 使用 Segoe UI 作为拉丁界面字体，并回退到 Microsoft YaHei UI/微软雅黑显示中文；macOS 使用系统 UI 字体并回退到 PingFang SC/苹方。其他平台只保留 `system-ui` 回退，不作为深度适配或验收对象。

前端在 React 渲染前识别桌面平台，并通过根节点 `data-platform` 选择字体栈和显示参数。macOS 保持 11/13px 辅助字号和 CoreText 表现；Windows 使用 12/14px，恢复 DirectWrite/WebView 的平台默认平滑策略并提高辅助文字的中性色对比。本轮只替换字体来源，不同时重调字号令牌，以便通过实机比较判断系统字体在不同缩放下的清晰度。

主窗口采用“沉浸式账户/文件夹侧栏、邮件列表、邮件阅读器”三栏结构，不再存在横跨窗口的顶部工具栏。账户身份位于侧栏顶部：只有一个账户时保持静态身份，两个及以上账户时显示切换菜单，并附带简洁的账户运行状态。切换只清理当前文件夹、邮件选择和搜索，再读取目标账户的 SQLite 本地视图，不等待网络。新建邮件和草稿入口位于文件夹之前；手动收取位于“邮件文件夹”标题右侧，设置固定在文件夹列表底部，侧栏不提供独立退出菜单。文件夹父节点的名称和展开箭头是独立动作：名称进入文件夹，箭头展开或收起子节点。中栏显示文件夹名称、总数/未读数、当前已加载邮件过滤框和连续邮件列表；选中项由强调色左侧条和派生背景表达。列表时间按本机日历分级显示为当天 `HH:mm`、昨天、本年 `MM-dd` 或跨年 `yyyy-MM-dd`。右栏将星标、回复、回复全部、转发、归档、移动、复制、删除和更多操作统一为带提示和 ARIA 标签的图标按钮。

主工作区选择状态、Tauri 邮件事件和分栏尺寸分别由 `useMailboxSelection`、`useMailRuntimeEvents`、`usePaneLayout` 承载。账户切换通过最新 ref 影响发件成功事件筛选，不重建整组监听；监听卸载会处理已经完成和仍在注册中的异步 unlisten。分栏 resize 以函数式状态更新读取最新两栏宽度，避免窗口缩放使用陈旧闭包。

独立设置窗口的“账户”分类提供账户列表、添加、连接编辑、重新认证、同步策略、文件夹映射和安全移除。“写信”分类提供全局/账户模板与签名库的富文本管理；通知和高级类别仍为稳定占位。首次启动与设置窗口复用同一个密码账户表单和发现/手动配置流程，不维护两套连接验证逻辑。移除最后一个账户后保留已经完成的数据目录初始化，主窗口展示正式的添加账户入口，不重新进入数据目录向导。

文件夹栏和邮件列表可在最小/最大宽度内拖动。分栏在布局中只占一个像素，宽命中区覆盖在相邻栏之上，hover/键盘聚焦时显示贯穿工作区的主题自适应细线，因此不在连续邮件列表两侧制造空隙；文件夹栏可折叠为保留可访问名称的图标模式。主列表与文件夹栏隐藏 WebView 原生滚动条，使用绝对定位的自绘滑块；文件夹栏及移动/复制菜单把滑块放进组件外围既有 padding，列表项主体宽度不受影响。滑块仅在实际滚动时短暂显示。当前搜索只过滤已加载邮件，不等同于后续 FTS 全文搜索。设置不再使用主窗口模态框，而是独立分类窗口；通用、外观、账户、阅读、写信模板/签名库和关于已接入现有能力，通知和高级类别仍提供稳定占位。

业务页面消费拆分后的布局、文本、表单、选择器、提示和空状态组件，原则上不直接使用原生表单控件。中文与英文文案由独立 JSON 语言包提供，不在功能组件中写死生产文案。首次设置保留语言切换；进入主界面后，语言、主题和强调色统一由独立设置窗口承载。

### 已收邮件附件

- 阅读器只把账户 ID 和附件 ID 提交给 Rust。Rust 验证匿名账户槽归属后，按需下载内容并生成安全缓存副本；前端不会获得内容哈希或文件路径。
- 普通附件通过 Tauri 官方 opener 的 Rust API 交给系统默认程序；该插件能力不写入前端 Capability。高风险扩展名只在系统文件管理器中显示，不直接执行。
- “下载后自动打开”是设备级阅读偏好且默认开启；关闭后首次点击只下载，已下载附件仍可再次点击打开。
- “另存为”由 Rust 发起系统保存对话框并复制已验证的缓存文件，公共命令不接收任意源路径。
- 文件名移除路径分隔符、控制字符、Windows 非法字符和保留设备名，并限制 UTF-8 长度；缓存目录只使用附件 ID 与内容哈希的摘要。

## HTML 阅读器

- Ammonia 先移除脚本、表单、嵌入文档、事件属性、危险 URL 与外部样式表；独立 CSS 模块随后用 `cssparser` 重建 `<style>` 和行内声明，只保留展示属性、普通/属性选择器与受控 `@media`。清洗器保留 `class`、`id`、传统表格宽高/居中/间距/对齐/背景色和字体属性，使常见邮件 HTML 的作者样式与固定宽度布局继续生效。网络 `url()`、未知函数、其他 at-rule、固定遮罩、动画和变换继续移除。
- 阅读器不再注入会改变作者几何布局的统一字体/行高、16px 内边距、任意断词或 `img/table max-width`；安全的远程 `<img>` URL 可以保留，但默认 iframe CSP 的 `img-src data:` 阻止请求。
- 清洗层只接受规范化后的 `http`、`https`、`mailto` 并直接保留为 `href`，固定设置 `target="_blank"` 与 `rel="noopener noreferrer"`；其他 scheme、相对/本机路径、用户信息、控制字符和混淆 URL 移除。
- 邮件 iframe 的 sandbox 只有 `allow-popups`。主窗口由既有 Tauri 平台配置显式创建，`on_new_window` 对目标再次执行 Rust URL 校验，再交给 `state.rs` 注入的系统外链打开器，并始终返回 `Deny`，因此外部网页不会在 NextMail WebView 内创建或加载；React 无链接事件、确认 UI 或通用 opener Command。
- “立即显示”或设备级“自动加载远程图片”只把当前 iframe 的图片 CSP 扩为 `data: http: https:`；sandbox 仍不启用 scripts、forms、same-origin 或 top-navigation，并使用 `no-referrer`。自动加载默认关闭，设置界面说明打开跟踪风险。
- Tauri 顶层 CSP 允许图片协议只是为 iframe 的显式选择提供上限；默认阻止由邮件文档自身更严格的 CSP 执行。
- 阅读 iframe 不继承应用 DOM 样式。NextMail 根据有效主题在 iframe 元素和内部文档设置 `color-scheme`，并注入不带 `!important` 的浅色或灰黑深色兜底；无明确样式的正文获得可读配色，邮件作者在页面、类或行内明确设置的颜色和背景按正常层叠优先。完整 HTML 的 `<body style>` 在清洗前转换为带固定标记的内部正文容器，经相同 CSS 过滤后保留页面级行内配色；该容器不增加脚本或 IPC 能力。`cid:`/附件资源协议与远程样式资源仍未实现。
- HTML 清洗策略升级时通过嵌入式迁移失效旧 HTML 正文缓存。迁移 0011 是未通过实机验收的链接映射原型且因 SQLx 校验不可修改；数据格式版本 12 的迁移 0012 删除临时 `message_links` 表并再次失效旧 `safe_html`。正文请求先按账户槽读取本地原始 EML，在不持有 SQLite 写锁的 blocking worker 中重新解析/清洗，再以单个事务写回正文与消息可用状态；只有本地原文缺失或不可解析时才通过 IMAP 获取。
- `testdata/mail-rendering/` 是 Rust/前端共享的正式保真与主动内容语料。ADR 0008 保留不透明 origin，sandbox 仅为受宿主拦截的用户链接点击增加 `allow-popups`；不增加脚本、表单、same-origin、顶层导航、前端通用网络或通用 opener 权限。
