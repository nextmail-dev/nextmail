# NextMail 架构基线

## 运行边界

NextMail 使用单个 Tauri 进程。React 仅通过稳定的 Tauri Command DTO 读取本地视图或提交业务命令，不直接连接 SQLite、邮件服务器、文件系统或系统凭据库。

主窗口启动遵循“首帧和本地视图优先”顺序：Tauri `setup` 只装配状态，不启动邮件同步或发件 Worker；React 首先显示随 HTML 内嵌的中性加载层，再读取 Bootstrap、外观配置和 SQLite 本地视图。主工作区完成至少一个绘制周期后，通过幂等业务命令启动后台服务。同步不会作为进入主界面的前置条件；全部服务器消息先逐封落库头部，正文只在用户打开邮件时按需取得。

Rust 代码只使用 `src-tauri` 下的单一 Cargo package，避免仓库根目录和 Tauri 目录各自产生一套 `Cargo.lock` 与 `target`：

- `src-tauri/src/core`：不依赖 Tauri、数据库和具体协议库的领域 DTO、稳定错误与 ports。
- `src-tauri/src/storage`：共享 SQLite/内容存储之上的读取、当前文件夹全文搜索、同步写入、模板/签名、草稿、发件任务、待办操作、文件夹角色及文件夹结构/排序子仓库；同步写入、文件夹管理与发件任务分别有窄实现文件，嵌入式迁移位于 `src-tauri/migrations`。
- `src-tauri/src/protocols`：IMAP 同步与写操作、MIME 解析/生成和 HTML 清洗；IMAP 内部按 Provider、连接、账户会话预算、mailbox 路径锁、具体会话操作、解析、文件夹编码和超时流拆分。新增协议 Adapter 只有在未来单独排期后进入此边界。
- `src-tauri/src/application`、`adapters`、`commands` 与运行时模块：首次启动用例、Keyring、自动发现、Command/Event、窗口和 Worker 装配；MailRuntime 的正文、待办、事件支撑，以及 ComposerRuntime 的定义和发件调度均为同层子模块。

仓库根目录不放置 Cargo manifest、lockfile 或 Rust 构建目录。唯一的 `Cargo.toml`、`Cargo.lock` 和 `target` 均由 `src-tauri` 管理。

协议库类型不得越过 Adapter。命令错误只返回稳定错误码、可本地化参数和是否可重试，不返回密码、服务器原始响应或内部堆栈。`core` 保持不依赖 `tracing`；宿主在进程启动时为所有 `CommandError` 构造安装只读观察器，只记录错误码、retryable 和调用位置，明确不接收 `params`。Rust panic、协议/存储原因链、后台任务与关键窗口/Event 失败，以及前端未捕获异常写入同一按日滚动本地日志。

### 窗口与 Capability

- Windows 主窗口和动态创建的写信/设置/账户管理/原始邮件窗口关闭系统 decorations，由 React 标题栏提供拖动、最小化、最大化和关闭按钮。
- macOS 保留系统 decorations，使用 `Overlay` 标题栏和系统默认定位的原生交通灯；React 只提供可拖动内容区，不伪造窗口按钮或硬编码交通灯坐标。
- 自绘标题栏使用紧凑高度；Windows 窗口按钮和 macOS 交通灯只保留满足拖动与原生操作所需的最小安全空间。站内通知通过根节点 Portal 渲染在标题栏下方，避免被工作区裁剪或窗口拖动层遮挡。
- `main`、`composer-*`、`settings`、`accounts`、`raw-message` 和 `notification-*` 使用独立 Capability。普通独立窗口控制只开放启动拖动、最小化、切换最大化、关闭、就绪后的自身显示/聚焦及需要释放单例 WebView 的销毁；写信窗口因发送成功需要绕过关闭拦截，同样保留 `allow-destroy`。通知窗口只开放 Tauri 事件监听/卸载与就绪后的自身显示，不获得聚焦、文件、网络、数据库、对话框、Shell、系统 opener 或任意建窗权限；业务交互限于与自身 label 绑定的 Bootstrap、关闭、激活命令和宿主定向事件。
- 设置、账户管理和原始邮件分别使用单例 `settings`、`accounts`、`raw-message` WebView。重复打开只聚焦已有窗口；原始邮件复用时仅发送公开账户/邮件 ID，窗口重新通过稳定 Command 读取原始 EML，不在事件中传正文。动态窗口先隐藏创建，React 完成懒加载和首批 Query 后通过窄窗口控制能力显示；错误边界同样会显示，从而避免把加载占位暴露给用户或在失败时留下永久隐藏窗口。偏好变化由 Rust 持久化后发布窄事件，各窗口把 DTO 写入各自的 TanStack Query cache 并更新主题和语言，不共享 React 内存状态。外观写入使用乐观 cache 更新，失败时恢复旧值。
- 尺寸、位置和最大化状态由 Rust 侧 Tauri 官方 `window-state` 插件写入系统应用配置目录，不进入可迁移邮件数据集。`main`、`settings`、`accounts`、`raw-message` 和 `composer` 各类状态分开；动态写信标签通过插件 label mapper 汇总到 `composer`。临时 `notification-*` 标签由 filter 排除，不进入状态文件。普通窗口无历史状态时居中，有历史状态时在隐藏创建后恢复并显示，前端无需窗口状态 IPC 或插件权限。
- `NotificationRuntime` 在 Rust 侧拥有窗口队列、超时和位置事实。候选只在同步成功后进入运行时；同一同步批次先按展示模式裁剪，层叠只保留最后 `X` 封、覆盖只保留最后一封，再统一创建和排布，避免高速淘汰造成闪动。窗口按主窗口所在显示器的物理工作区与 DPI 从右下角向上层叠，覆盖模式复用同一窗口并用 generation 使旧超时失效。通知 React 完成 Bootstrap 后才显示，不主动抢焦点，也不进入任务栏。

### Rust 模块拆分策略

NextMail 不再用多个 Cargo package 表达业务边界，而是在单一 `src-tauri` package 内保持清晰模块：

- `core`：纯 Rust 的领域模型、用例接口和稳定错误。
- `protocols`：当前 IMAP、SMTP、MIME 和 HTML 安全 Adapter。
- `storage`：SQLite、原始邮件、附件和索引存储。
- Tauri 宿主模块：窗口、Capability、系统集成、Command/Event 和运行时装配。

依赖方向仍保持为宿主和 Adapter 指向核心，核心不得依赖 Tauri、SQLx 或具体协议库。协议库与 SQLx 类型不得越过模块边界；模块级单元测试、公共 DTO 审查和受控可见性用于维持原有隔离。除非未来出现独立发布、独立版本或被其他二进制复用的实际需求，不再为形式上的分层创建 Cargo Workspace 或子 crate。

账户、Bootstrap 与本机偏好的配置读写以 `core::ports` 注入 application service；IMAP Provider、Repository Provider、系统附件、外链打开能力与通知运行时同样由 `state.rs` 装配。Application 不构造具体 JSON Store，Worker 不构造具体 IMAP/SQLite Adapter。写信与邮件运行时复用同一个 Repository 实例和 SQLite 连接池；邮件运行时只把已经过偏好过滤的新邮件候选交给注入的通知运行时。模板与签名输入校验、变量渲染及初始三格式组合位于 application，SQLx Repository 只按显式作用域持久化定义、场景引用和 revision。

## 存储边界

用户选择的数据目录是可迁移数据集，当前包含：

- `.nextmail-data.json`：格式版本和匿名数据集 ID。
- `content.sqlite`：匿名账户槽、文件夹、邮件、远端位置、正文、FTS5 搜索视图、模板、签名、草稿、附件元数据、发件任务、待办操作与同步状态。
- `raw/`：按 SHA-256 分层保存的收取和待发送原始 EML。
- `attachments/`：按 SHA-256 分层、去重保存的已下载附件和草稿附件副本。
- `cache/`：可重建缓存的保留目录。

已下载邮件附件仍以无扩展名的内容哈希保存在 `attachments/`。用户打开附件时，Rust 在 `cache/attachment-open/` 下按不透明附件 ID 与内容哈希生成带安全文件名的可重建副本；原始数据路径和缓存路径都不返回 React。

邮箱地址、服务器配置、数据槽映射、首次启动状态、外观设置、阅读偏好、通知偏好和不含业务数据的窗口几何状态位于 Tauri 系统应用配置目录。`accounts.json` 使用单调修订号、进程内串行变更锁和原子文件替换维护多账户集合；最近选择账户与不含邮箱或秘密的待清理凭据引用也保存在此。阅读偏好与通知偏好分别原子写入 `reading-preferences.json` 和 `notification-preferences.json`，不会随邮件数据目录迁移；通知文件只保存公开账户/文件夹 ID、开关和展示参数。窗口状态由官方插件维护，也不属于跨机邮件数据迁移范围；密码只以 `com.taurusxin.nextmail` 服务项写入 Windows Credential Manager 或 macOS Keychain。

数据目录初始化只接受空目录或兼容的 NextMail 目录。新建过程失败时仅清理本次创建的标记、数据库和空子目录，不递归删除用户原有内容。

## 连接安全

- 全进程统一使用 rustls `ring` CryptoProvider，并在 Tauri 初始化前显式安装；直接 TLS 依赖关闭默认 provider 特性，避免依赖合并后出现 provider 歧义。
- IMAP 支持无加密、STARTTLS 和隐式 TLS；TLS 使用系统根证书并严格校验主机名。IMAP 同步与首次账户连接测试共享进程级 rustls 配置，系统根证书只在首次 TLS 连接时加载一次。
- SMTP 使用 lettre、Tokio 和 rustls；连接测试只认证账户，正式发件使用持久化 MIME 和 `send_raw`。
- 无加密连接必须由用户显式确认，后端在保存时再次校验该确认。
- 自动发现顺序为内置服务商、DNS SRV、域名 HTTPS autoconfig。自动配置响应限制为 1 MiB 且不接受 HTTP 降级。
- 新增账户按连接验证、匿名数据槽、系统凭据、外置账户配置的顺序提交；任一步失败都会补偿此前写入。编辑密码先写新凭据引用，再把新配置和旧引用清理任务原子提交，最后幂等清理旧凭据。移除账户同样先把配置移除与清理任务一起提交，凭据库临时失败不会恢复已移除账户或留下明文秘密。

## IMAP 同步与离线操作

- `MailRuntime` 作为 Supervisor Registry，按 `account_id` 维护至多一个 `AccountSupervisor`。每个账户独立拥有启动同步、周期计时、手动同步和待办重放状态；所有账户共享一个 Repository/SQLite 连接池，并始终通过匿名 `account_slot_id` 隔离数据。
- 高层主动 IMAP 操作共享两个全局许可；具体 Provider 再按账户限制最多 3 条主动 IMAP 会话。完整同步只租用 2 条 worker 会话，保留第 3 条给正文、附件或待办操作，因此打开正文不会为同一账户建立第 4 条连接或取消同步。连接按操作建立并关闭，预算弱引用在账户空闲后自然回收，`async-imap::Session` 不越过 Adapter。
- 文件夹创建、重命名、层级移动、删除和全部已读复用第 3 条交互会话容量。结构操作必须在线完成：MailRuntime 只向 core port 提交公开账户解析后的服务器路径上下文与 Unicode 叶名称，具体 Adapter 负责 modified UTF-7 编码并执行 `CREATE`/`RENAME`/`DELETE`/`UID STORE`。Adapter 的账户级 mailbox 路径读写锁让同步、正文、消息待办和 APPEND/草稿替换继续共享并发读锁，只让结构操作/全部已读等待并独占路径写锁，避免旧路径或旧 Flags 快照竞态；锁不跨 SQLite 访问。服务器成功后，独立 `MailboxRepository` 才事务更新本地投影；数据库事务不跨网络 await。该边界和本地排序决策见 ADR 0015。
- Supervisor 只在主工作区完成首帧后启动；启动同步在内存中预先进入 `connecting` 状态，进度查询即使错过最早事件也能读到当前阶段。运行时启动和发件 Worker 启动均为幂等操作，可安全承受 React Strict Mode 或窗口状态变化导致的重复通知。
- 同步按 UIDVALIDITY/UID 定位邮件，先计算远端 UID 与已落库 UID 的差集，只为缺失消息按最多 100 个 UID 的网络批次 FETCH 头部，并对账当前 UID 集合、Flags 和 MODSEQ。文件夹对账通过事务内远端 UID 集合做集合删除，不按本地位置逐行查询。UIDVALIDITY 改变时重建文件夹位置，不使用消息序号作为持久身份；同步中断后从已落库 UID 继续。
- 用户修改在 SQLite 事务中同时更新本地投影和 `pending_operations`。Worker 按顺序执行，`running` 状态可在重启后恢复。
- Flags 以 `.SILENT` 增量 STORE 写回，只改变目标命名 flag，不依赖部分服务器缺失的 FETCH 响应；CONDSTORE/MODSEQ 仍用于同步侧状态对账，不把条件 STORE 兼容性风险带入待办重放。
- MOVE、UIDPLUS 和 CONDSTORE 全部在自有 Adapter 内做 Capability 分支。缺失 UIDPLUS 时不执行可能影响其他邮件的宽泛 EXPUNGE。
- React 事件只收到账户、文件夹、消息或操作 ID、同步文件夹末级显示名与修订状态，并通过 TanStack Query 重新读取本地视图。邮件详情 key 统一为账户、文件夹、消息四段；缺少文件夹 ID 的正文事件按账户前缀失效，避免把消息 ID 放入错误槽位。同步显示名按 IMAP 服务器声明的层级分隔符从完整 Unicode 名称提取，只用于可见手动/启动同步状态，不改变真实邮箱定位且不携带邮件内容。同步进度卡不显示文件夹完成数/总数，只在摘要阶段显示当前文件夹内已同步邮件数/总数。
- 网络读取仍在异步 IMAP 会话中完成；头部/MIME 解析和 HTML 清洗使用 Tokio blocking worker，避免占用异步调度线程。网络每批最多获取 100 个 UID，但每封头部完成原子落库后立即发布 `message-arrived`，既不等待整批，也不等待整个文件夹或账户同步结束；前端只在 100ms 窗口合并当前文件夹增量渲染。
- 完整同步不自动下载正文。打开缺少正文的邮件时优先从账户槽内已有原始 EML 离线重建，否则使用保留的第 3 条账户会话容量获取完整 MIME；手动正文事件只传 ID、阶段与进度，正文仍由 Query 重新读取。
- `account_sync_settings` 当前只使用同步间隔，允许手动、1、5、10 分钟且默认 1 分钟；阶段十二的正文时间与非收件箱正文列只为兼容已有数据库保留，不再进入 DTO、设置 UI 或同步决策。间隔写入只唤醒 Supervisor 重置计时。
- 单封邮件的规范记录、远端位置、正文和附件元数据在同一 SQLite 事务内提交，附件元数据使用批量 UPSERT。内容寻址原始 EML 在事务前完成幂等文件写入，数据库失败不会留下可见的半成品邮件。
- 新邮件候选以“新建远端位置”为准，正文回填不产生候选。匿名账户槽持久化首次完整同步基线；新账户首次同步、新文件夹初次发现和 UIDVALIDITY 重建抑制历史邮件。候选只在整次账户同步成功后按全局/账户/文件夹偏好过滤并去重，再发布包含公开 ID、发件人与主题的最小事件；默认只启用 Inbox 未读邮件。偏好读取和事件发布发生在 SQLite 事务之外。
- Supervisor 区分“仅执行待办”和“执行完整同步”：本地 Flags、移动、复制、删除及 APPEND 只唤醒待办 Worker。完整同步只由账户 Supervisor 首次创建、配置的周期计时到期或用户手动收取触发；账户首次设定和应用启动都会创建 Supervisor，因此各执行一次启动同步。
- 不使用 Inbox IDLE、无 IDLE 轮询、固定兜底同步或秒级失败重试。同步失败保持离线/重新认证状态，等待下一次已配置计时或手动重试，避免多个触发源形成加载循环。

## 草稿与发件边界

- Composer 工具栏选择和剪贴板图片只把用户明确提供的受限字节交给 `add_draft_inline_image`，由 Rust 验证并写入受管内容存储。富 HTML 剪贴板内容先通过窄 `sanitize_rich_text_paste` Command 清洗并限定到 `data-nextmail-pasted-html` 容器作用域，再由 Tiptap 保留安全 class/ID、行内样式和样式表；未经处理的 HTML 不进入 Composer DOM，远程图片不会因此静默下载。

- 模板与签名使用独立窄 Repository 保存三格式富文本定义、四场景模板规则和单一默认签名偏好。全局定义/偏好使用空账户槽，账户记录始终通过 Rust 将公开账户 ID 解析为匿名 `account_slot_id`；React 不接触数据槽。账户模板规则和签名偏好没有显式记录时分别继承全局值；创建某范围第一个签名时，签名和默认偏好在同一 SQLite 事务内提交。引用范围与 revision 在 Repository 边界验证。
- 变量白名单、缺失上下文错误、HTML/主题/纯文本差异化转义与本地化日期在 application 完成。设置窗口和 Composer 只通过 `src/app/api.ts` 使用稳定 DTO；Composer 先取得可见定义摘要，再把当前收件人上下文交给 Rust 渲染。
- Tiptap 使用 `nextmailTemplate` 和 `nextmailSignature` 可编辑块节点保存定义 ID，HTML 使用对应 `data-nextmail-*-id` 属性。模板按四个场景解析，签名从账户覆盖/全局继承的单一偏好解析；自动选择开关只影响草稿首次创建，因此用户手动删除签名后自动保存或重开不会恢复。回复/转发草稿另以 `nextmailReply` 和原子 `nextmailOriginalMessage` 固定用户回复与原文边界；原文内部 HTML 不进入 ProseMirror schema，模板递归定位在回复区，签名固定在原文前，切换定义不会跨边界改写原始内容。
- 独立 `composer-*` WebView 通过窄业务命令访问草稿，不直接访问数据库、任意文件或网络；发件人是只读标签，收件人标签在分隔符/失焦时提供即时语法反馈，空输入退格可恢复末尾标签继续编辑。存在尚未提交的收件人文本时不会由自动保存定时器静默转成标签，发送或关闭前仍由前端最终提交并由 Rust 发件校验。系统文件选择器只授权用户明确选择的普通附件，剪贴板图片只把受限字节交给 `add_draft_inline_image`，由 Rust 验证并写入受管内容存储。
- 草稿保存 Tiptap JSON、HTML 和纯文本，使用修订号做乐观并发控制；CodeMirror 源码编辑仍产生同一三格式 DTO，Rust 在持久化前复验 HTML/JSON。写信窗口关闭前会提交未保存改动；关闭监听按账户与草稿身份单次订阅，并通过 ref 读取最新保存函数与编辑状态。
- SMTP 联网前先用 `mail-builder` 生成完整 UTF-8 MIME，按内容哈希原子落盘并创建 `send_job`。正文为 `multipart/alternative`，CID 图片与 HTML 组成 `multipart/related`，普通附件存在时再组成 `multipart/mixed`。MIME `Date` 头在生成时读取操作系统本机时区并写入当时的 UTC 偏移；Bcc 只进入 SMTP envelope，不写入邮件头。
- 后台 `SendWorker` 从系统凭据库取密码，按账户内 FIFO、账户间轮转方式发送不可变 MIME；全局最多同时发送两封，同一账户同时最多一封。单个账户的超时、断网或认证错误不会阻塞其他账户；临时错误最多自动尝试三次，失败内容继续保留并支持显式重试。
- 异常退出遗留的 `sending` 在启动时恢复为 `queued`。SMTP 成功后独立排队 APPEND 到映射的 Sent；Sent 归档失败不会触发再次 SMTP 发送。
- 本地草稿停止编辑 10 秒或关闭窗口时排队同步到映射的 Drafts。远端版本用 `X-NextMail-Draft-ID` 关联，先追加新版本再安全清理旧 UID；服务器草稿可转换成本地可编辑草稿。
- Tiptap 写信代码按窗口动态加载，不进入主窗口首包。
- 完全空白草稿以及从未发生用户保存的回复/回复全部/转发动作草稿在写信窗口关闭时由 Rust 条件删除；删除结果返回前端后才决定是否排队远端 Drafts。远端导入草稿和已编辑动作草稿不会命中该条件，前端也不能直接删除任意草稿。SMTP 成功通过 ID/状态事件通知主窗口，由主窗口显示站内成功通知。
- 回复、回复全部和转发由 application 层的纯用例从本地规范邮件生成新草稿，Repository 只读取源邮件并持久化组合结果。回复草稿保存 `In-Reply-To`/`References` 并在 MIME 生成时安全注入；回复全部排除自身并去重，转发按需取得原附件后复用内容寻址副本。

## 邮件与文件夹编码

- RFC 2047 邮件头、结构化地址、MIME 正文和附件名统一由启用 `full_encoding` 的 `mail-parser` 解码；NextMail 不维护第二套 encoded-word 或字符集解析器，只保留领域 DTO 映射与回归语料。
- 支持 GB2312/GBK/GB18030、Big5、Shift-JIS、EUC-KR、Windows code pages、ISO-8859 系列和 Unicode 编码；未知或畸形 RFC 2047 encoded-word 保留原文并继续解析后续字段，不用系统区域设置猜测。
- IMAP 远端文件夹名保留线缆原值用于 `EXAMINE` 和结构操作，另生成 modified UTF-7 解码后的 Unicode 显示名，避免显示名反向影响协议定位。创建/重命名时只有用户输入的 Unicode 叶名称在 Adapter 内编码为 modified UTF-7；父路径和服务端分隔符取自本地 mailbox 上下文。服务端返回的层级分隔符随文件夹 DTO 传给界面；文件夹树只按该分隔符连接已存在的父节点，不猜测名称中的 `/`、`.` 等字符。
- 标准文件夹由 `MailboxRole` 本地化，用户创建的其他文件夹保留服务端名称语义。

## 前端设计系统

账户运行状态事件会立即失效主工作区 runtime Query，使启动、定时或手动同步期间都禁用手动收信；Rust 同时拒绝竞态重复请求。邮件仍按最多 100 UID 网络批取并逐封落库，每次 `mailbox-changed` 都为当前文件夹串行排队一次真实本地视图重读，后一封不会取消前一封刷新，也没有前端定时模拟播放。

设置侧栏、文件夹和其他选中项统一从主题色派生淡色背景与前景色，不使用固定白色高亮。

前端采用 shadcn 的源码归属模式而不是安装黑盒组件库：组件源码位于 `src/components/ui/`，每个组件独立文件，可按产品需求修改。Radix 只提供无样式的键盘、焦点和 ARIA 行为。

主题使用 shadcn 语义 CSS Variables，并通过 Tailwind v4 映射为工具类。未保存外观偏好时使用浅色作为产品默认值，系统、浅色和深色仍可显式选择，已有持久化选择不被默认值变化覆盖。当前视觉基线为现代 SaaS 风格：浅色主题使用清新的白色与中性灰表面，深色主题使用无色度灰黑表面；用户可见的“主题色”在内部作为强调色令牌，独立派生选中背景、焦点环和主操作。控件以背景、留白、阴影和文字层级表达状态，普通按钮、输入框、弹层、导航项和内容区域不绘制装饰性边框。基础圆角为 10px，保留清晰几何而不使用拟物效果。

UI 使用操作系统原生字体栈，不再随 Vite 打包字体。Windows 使用 Segoe UI 作为拉丁界面字体，并回退到 Microsoft YaHei UI/微软雅黑显示中文；macOS 使用系统 UI 字体并回退到 PingFang SC/苹方。其他平台只保留 `system-ui` 回退，不作为深度适配或验收对象。

前端在 React 渲染前识别桌面平台，并通过根节点 `data-platform` 选择字体栈和显示参数。macOS 保持 11/13px 辅助字号和 CoreText 表现；Windows 使用 12/14px，恢复 DirectWrite/WebView 的平台默认平滑策略并提高辅助文字的中性色对比。本轮只替换字体来源，不同时重调字号令牌，以便通过实机比较判断系统字体在不同缩放下的清晰度。

主窗口采用“沉浸式账户/文件夹侧栏、邮件列表、邮件阅读器”三栏结构，不再存在横跨窗口的顶部工具栏。账户身份位于侧栏顶部并始终打开账户菜单，即使只有一个账户；菜单上部切换账户，底部经分割线进入账户管理。账户身份旁不附加同步状态文字，当前手动/启动同步文件夹显示在文件夹区进度卡，离线、重试和重新认证等可操作状态仍显示。切换只清理当前文件夹、邮件选择和搜索，再读取目标账户的 SQLite 本地视图，不等待网络。新建邮件和草稿入口位于文件夹之前；手动收取位于“邮件文件夹”标题右侧，设置固定在文件夹列表底部，侧栏不提供独立退出菜单。文件夹父节点的名称和展开箭头是独立动作：名称进入文件夹，箭头展开或收起子节点。文件夹右键菜单提供创建子级、重命名、服务器层级移动、删除和全部已读；“邮件文件夹”标题的右键菜单创建根级文件夹。展开栏长按文件夹 360ms 后进入本地拖拽，只接受同一父级目标并按目标上下半区插入，父级携带完整下级子树；跨层拖拽不产生操作，服务器层级只能通过菜单改变。首次本地排序后完整顺序按 `account_slot_id` 写入 SQLite，重启或完整同步不覆盖。中栏显示文件夹名称、总数/未读数、当前文件夹本地全文搜索框和连续邮件列表；搜索覆盖尚未加载到 React 的本地页，选中项由强调色左侧条和派生背景表达。再次单击选中项会取消选择；移动、归档或删除当前项后优先选中其下一项，列表末尾回退上一项。列表右键菜单覆盖回复、回复全部、转发、编辑服务器草稿、已读/未读、星标、归档、移动、复制和删除，并复用阅读器的命令路径。列表时间按本机日历分级显示为当天 `HH:mm`、昨天、本年 `MM-dd` 或跨年 `yyyy-MM-dd`。右栏将星标、回复、回复全部、转发、归档、移动、复制、删除和更多操作统一为带提示和 ARIA 标签的图标按钮。

主工作区选择状态、Tauri 邮件事件、文件夹 mutation 和分栏尺寸分别由 `useMailboxSelection`、`useMailRuntimeEvents`、`useMailboxActions`、`usePaneLayout` 承载；长按指针状态由独立 `useMailboxReorderGesture` 管理，纯排序函数负责同层/子树约束。账户切换通过最新 ref 影响发件成功事件筛选，不重建整组监听；监听卸载会处理已经完成和仍在注册中的异步 unlisten。通知点击由 Rust 先核验账户/文件夹/邮件目标并为有效消息排入已读更新，再发出定向事件；选择 hook 在文件夹 Query 就绪后选择仍可见的邮件，失效消息不保留陈旧选择。分栏 resize 以函数式状态更新读取最新两栏宽度，避免窗口缩放使用陈旧闭包。

独立账户管理窗口提供账户列表、添加、连接编辑、重新认证、手动/1/5/10 分钟自动同步间隔、文件夹映射和安全移除；账户列表与详情面板仍由 TanStack Query 驱动，账户或运行状态变更只失效对应 key。主窗口账户菜单和无账户空状态只调用稳定 Rust 建窗命令，不渲染管理弹层。独立设置窗口不再保留重复的“账户”分类，“写信”分类继续提供全局/账户模板与签名库的富文本管理；“通知”分类提供全局开关、展示模式/数量/时间、逐账户开关及三点菜单内的逐文件夹开关。偏好写入会关闭当前瞬时通知，后续候选使用新设置；高级类别仍为稳定占位。首次启动与独立账户管理窗口复用同一个密码账户表单和发现/手动配置流程，不维护两套连接验证逻辑。移除最后一个账户后保留已经完成的数据目录初始化，主窗口展示打开同一独立管理窗口的正式添加入口，不重新进入数据目录向导。

文件夹栏和邮件列表可在最小/最大宽度内拖动。分栏 grid 轨道为零宽，宽命中区覆盖在相邻栏内部，hover/键盘聚焦时显示贯穿工作区的主题自适应细线，因此三栏之间没有 1 px 物理空隙；文件夹栏可折叠为保留可访问名称的图标模式。应用纵向滚动容器隐藏 WebView 原生滚动条，统一使用绝对定位的自绘覆盖滑块；滑块位于组件已有 padding/margin，不保留 `scrollbar-gutter`，主体宽度不受影响。自绘滑块默认在内容可滚动时常驻，只有文件夹列表显式启用自动隐藏并在 hover/键盘焦点进入时显示。邮件列表透明拖动命中区加宽并向列表内部移动，避免误触右侧分栏 ResizeHandle。当前搜索经 250ms 防抖调用 Rust `search_messages`，使用 FTS5 查询当前账户/文件夹的本地主题、地址、预览、纯文本正文与附件名；结果沿用邮件列表分页，前端不再二次筛选。设置未加载完成时中性回退覆盖整个 WebView，不先显示标题栏侧栏接缝；通用、外观、阅读、写信模板/签名库、通知偏好和关于已接入现有能力，高级类别仍提供稳定占位。

业务页面消费拆分后的布局、文本、表单、选择器、提示和空状态组件，原则上不直接使用原生表单控件。应用壳和展示性 UI 默认禁止文本选择，避免打包后仍呈现网页式拖选；输入框、富文本、CodeMirror、邮件标题、收发件人地址、邮件纯文本/HTML 正文与原始源码显式保留选择能力。中文与英文文案由独立 JSON 语言包提供，不在功能组件中写死生产文案。首次设置保留语言切换；进入主界面后，语言、主题和强调色统一由独立设置窗口承载。

### 已收邮件附件

- 阅读器只把账户 ID 和附件 ID 提交给 Rust。Rust 验证匿名账户槽归属后，按需下载内容并生成安全缓存副本；前端不会获得内容哈希或文件路径。
- 普通附件通过 Tauri 官方 opener 的 Rust API 交给系统默认程序；该插件能力不写入前端 Capability。高风险扩展名只在系统文件管理器中显示，不直接执行。
- “下载后自动打开”是设备级阅读偏好且默认开启；关闭后首次点击只下载，已下载附件仍可再次点击打开。
- “另存为”由 Rust 发起系统保存对话框并复制已验证的缓存文件，公共命令不接收任意源路径。
- 文件名移除路径分隔符、控制字符、Windows 非法字符和保留设备名，并限制 UTF-8 长度；缓存目录只使用附件 ID 与内容哈希的摘要。

## HTML 阅读器

- Ammonia 先移除脚本、表单、嵌入文档、事件属性、危险 URL 与外部样式表；独立 CSS 模块随后用 `cssparser` 重建 `<style>` 和行内声明，只保留展示属性、普通/属性选择器、受限 An+B 的四种 `nth-*()` 结构伪类与受控 `@media`。清洗器保留 `class`、`id`、传统表格宽高/居中/间距/对齐/背景色和字体属性，使常见邮件 HTML 的作者样式、固定宽度布局和 flex 表格列比例继续生效。网络 `url()`、其他选择器/声明函数、其他 at-rule、固定遮罩、动画和变换继续移除。
- 阅读器不向邮件文档注入会改变作者几何布局的统一字体/行高、内边距、任意断词或 `img/table max-width`；阅读栏只在 iframe 宿主外增加 12px 左右留白，不改写作者布局。安全的远程 `<img>` URL 可以保留，但默认 iframe CSP 的 `img-src data:` 阻止请求。
- 清洗层只接受规范化后的 `http`、`https`、`mailto` 并直接保留为 `href`，固定设置 `target="_blank"` 与 `rel="noopener noreferrer"`；其他 scheme、相对/本机路径、用户信息、控制字符和混淆 URL 移除。
- 邮件 iframe 的 sandbox 只有 `allow-popups`。主窗口由既有 Tauri 平台配置显式创建，`on_new_window` 对目标再次执行 Rust URL 校验，再交给 `state.rs` 注入的系统外链打开器，并始终返回 `Deny`，因此外部网页不会在 NextMail WebView 内创建或加载；React 无链接事件、确认 UI 或通用 opener Command。
- “立即显示”或设备级“自动加载远程图片”只把当前 iframe 的图片 CSP 扩为 `data: http: https:`；sandbox 仍不启用 scripts、forms、same-origin 或 top-navigation，并使用 `no-referrer`。自动加载默认关闭，设置界面说明打开跟踪风险。
- Tauri 顶层 CSP 允许图片协议只是为 iframe 的显式选择提供上限；默认阻止由邮件文档自身更严格的 CSP 执行。
- 阅读 iframe 不继承应用 DOM 样式。NextMail 根据有效主题在 iframe 元素和内部文档设置 `color-scheme`，并注入不带 `!important` 的浅色或灰黑深色兜底；无明确样式的正文获得可读配色，邮件作者在页面、类或行内明确设置的颜色和背景按正常层叠优先。完整 HTML 的 `<body style>` 在清洗前转换为带固定标记的内部正文容器，经相同 CSS 过滤后保留页面级行内配色；该容器不增加脚本或 IPC 能力。正文实际引用的受限本地 CID 图片由 Rust 转为 data URL 并受 `img-src data:` 约束；远程样式资源仍未实现。
- HTML 清洗策略升级时通过嵌入式迁移失效旧 HTML 正文缓存。迁移 0011 是未通过实机验收的链接映射原型且因 SQLx 校验不可修改；0012 删除临时表并失效旧 `safe_html`，0013 扩展草稿内嵌图片，0014 为安全选择器保真失效缓存，0015 回填本地搜索，0016 增加账户正文偏好与动作草稿生命周期标记，0017 新增签名偏好并收敛旧场景签名引用，0018 为账户增加通知同步基线，0019 增加每账户同步间隔，0020 增加按账户文件夹本地顺序，0021 为标准 CID 与受限 data 图片失效旧安全正文缓存。当前数据格式版本 22 的迁移 0022 为已应用 0021 的实机数据再次失效缓存，使误标为 `application/octet-stream`、但文件魔数可确认格式的 CID 图片重新解析；这些迁移不复制签名正文，也不把通知内容持久化。正文请求先按账户槽读取本地原始 EML，在不持有 SQLite 写锁的 blocking worker 中重新解析/清洗，再以单个事务写回正文、消息可用状态和已确认内联图片的附件投影；只有本地原文缺失或不可解析时才通过 IMAP 获取。
- Composer 不接收未经处理的邮件 HTML。回复/转发创建时优先在 blocking worker 中从账户槽内的原始 EML 提取 HTML part，缺失时回退到现有安全正文；compose 清洗器沿用主动内容与 URL/CSS 边界，并把保留的内嵌样式表重写到原文容器作用域。引用原文以 `sourceHtml` 原子节点保留，不进入会规范化表格的 ProseMirror schema；富文本视图和 HTML 源码实时预览都使用 `sandbox=""`、`no-referrer`、无脚本/表单/同源/导航权限的 iframe。原文 iframe 根据安全 HTML 的文本、表格行和可用内嵌图片估算无上限展开高度，关闭内部滚动并由外层 Composer 统一滚动；不会为读取 `scrollHeight` 放开脚本或同源。源码可由 CodeMirror 编辑，保存时 Rust 再清洗 HTML 与原文节点属性。
- 原始 MIME 中被 HTML `cid:` 引用的安全图片和用户粘贴图片写入既有内容寻址 `attachments/` 存储，`draft_attachments` 只记录 CID/inline 元数据。前端不读取文件系统路径或内容哈希，只使用 Rust 返回的内存 data URL 预览；持久化 HTML 使用 `cid:`，发件 MIME 使用 `multipart/related`。远程 `http(s)` 图片既不显示占位卡片也不在 Composer 中静默下载，仍受现有远程内容隐私策略约束。
- `testdata/mail-rendering/` 是 Rust/前端共享的正式保真与主动内容语料，包含合成的 `nth-child()` flex 发票表格。ADR 0008 保留不透明 origin，sandbox 仅为受宿主拦截的用户链接点击增加 `allow-popups`；不增加脚本、表单、same-origin、顶层导航、前端通用网络或通用 opener 权限。
