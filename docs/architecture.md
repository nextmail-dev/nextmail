# NextMail 架构基线

## 运行边界

NextMail 使用单个 Tauri 进程。React 仅通过稳定的 Tauri Command DTO 读取本地视图或提交业务命令，不直接连接 SQLite、邮件服务器、文件系统或系统凭据库。

Rust 代码使用 Cargo Workspace 分为：

- `crates/nextmail-core`：不依赖 Tauri、数据库和协议库的领域 DTO、稳定错误与 ports。
- `crates/nextmail-storage`：SQLx Repository、嵌入式迁移和内容寻址文件存储。
- `crates/nextmail-protocols`：只读 IMAP、MIME 解析/生成和 HTML 清洗；后续继续承载 POP3 Adapter。
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
- `content.sqlite`：匿名账户槽、文件夹、邮件、远端位置、正文、草稿、附件元数据、发件任务与同步状态。
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

## 草稿与发件边界

- 独立 `composer-*` WebView 通过窄业务命令访问草稿，不直接访问数据库、任意文件或网络；系统文件选择器只授权用户明确选择的附件。
- 草稿保存 Tiptap JSON、HTML 和纯文本，使用修订号做乐观并发控制。写信窗口关闭前会提交未保存改动。
- SMTP 联网前先用 `mail-builder` 生成完整 UTF-8 MIME，按内容哈希原子落盘并创建 `send_job`。Bcc 只进入 SMTP envelope，不写入邮件头。
- 后台 `SendWorker` 从系统凭据库取密码，串行发送不可变 MIME；临时错误最多自动尝试三次，失败内容继续保留并支持显式重试。
- 异常退出遗留的 `sending` 在启动时恢复为 `queued`。SMTP 成功只标记本地 `sent`，IMAP Sent/Drafts 归档留在第四阶段。
- Tiptap 写信代码按窗口动态加载，不进入主窗口首包。

## 邮件与文件夹编码

- RFC 2047 邮件头、结构化地址、MIME 正文和附件名统一由启用 `full_encoding` 的 `mail-parser` 解码；NextMail 不维护第二套 encoded-word 或字符集解析器，只保留领域 DTO 映射与回归语料。
- 支持 GB2312/GBK/GB18030、Big5、Shift-JIS、EUC-KR、Windows code pages、ISO-8859 系列和 Unicode 编码；未知或畸形 RFC 2047 encoded-word 保留原文并继续解析后续字段，不用系统区域设置猜测。
- IMAP 远端文件夹名保留线缆原值用于 `EXAMINE`，另生成 modified UTF-7 解码后的 Unicode 显示名，避免显示名反向影响协议定位。
- 标准文件夹由 `MailboxRole` 本地化，用户创建的其他文件夹保留服务端名称语义。

## 前端设计系统

前端采用 shadcn 的源码归属模式而不是安装黑盒组件库：组件源码位于 `src/components/ui/`，每个组件独立文件，可按产品需求修改。Radix 只提供无样式的键盘、焦点和 ARIA 行为。

主题使用 shadcn 语义 CSS Variables，并通过 Tailwind v4 映射为工具类。视觉基线结合 Nova 的紧凑密度和 Lyra 的利落几何：基础圆角为 4px，组件使用矩形或微圆角边框，不采用 shadcn 默认外观。`styles/theme.css` 保存令牌和明暗主题，`styles/base.css` 仅保存全局重置，页面布局与组件状态使用 Tailwind 类表达。

业务页面消费拆分后的布局、文本、表单、选择器、提示和空状态组件，原则上不直接使用原生表单控件。中文与英文文案由独立 JSON 语言包提供，不在功能组件中写死生产文案。首次设置保留语言切换；进入主界面后，语言、主题和强调色统一由工具栏菜单中的“设置”承载。
