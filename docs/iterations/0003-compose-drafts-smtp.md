# 第三阶段：写信、草稿与 SMTP 发件

状态：已实施并通过自动验证，等待用户手动验收。

## 实施结果（2026-07-12）

已完成独立 `composer-*` 写信窗口、主工具栏本地草稿下拉与重启续写、按需加载的 Tiptap 编辑器、自有富文本工具栏与文字样式、基本身份签名插入、三格式草稿自动保存、关闭前保存、空白草稿清理、系统附件选择、内容寻址附件副本、Unicode MIME 生成、Bcc envelope 隔离、持久化 `send_job`、后台 SMTP `send_raw`、三次有限自动重试、失败显式重试、主窗口成功通知和异常启动恢复。

本轮没有实现 IMAP Sent/Drafts 上传、可配置签名库、模板、多账户、OAuth 或完整发件箱页面。

自动验证：

- `cargo fmt --all -- --check`：通过。
- `cargo test --workspace`：通过，共 25 个 Rust 测试。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `pnpm test`：通过，共 6 个前端测试。
- `pnpm build`：通过；Tiptap 被拆分为独立 Composer chunk，未进入主窗口首包。
- `pnpm tauri build --debug --no-bundle`：通过，产物为 `target/debug/nextmail.exe`；构建缓存保留。

## 一、阶段目标

在现有单账户、本地优先阅读架构上完成“独立写信窗口 → 自动保存草稿 → 生成原始 MIME → 持久化发件任务 → 后台 SMTP 发送”的闭环。

- 工具栏“新建”打开独立写信窗口；一个草稿只允许一个活动窗口。
- 编辑器使用 MIT 开源版 Tiptap/ProseMirror，自有工具栏和 NextMail 设计令牌，不使用商业扩展或默认外观。
- 草稿同时保存编辑器 JSON、HTML 和纯文本，崩溃或重启后可以继续编辑。
- 附件由系统选择器明确授权，Rust 读取后按 SHA-256 存入可迁移数据目录；前端只使用不透明附件 ID。
- 发送前生成并持久化不可变原始 MIME 和 `send_job`，后台 Worker 从系统凭据库取密码并通过 SMTP `send_raw` 发送。
- 发送失败保留草稿、MIME 和错误状态，支持显式重试；应用重启后恢复未完成任务。

本阶段继续限制为首次启动时保存的单账户和密码认证。多账户、OAuth、模板/签名管理、IMAP Sent/Drafts 写入和离线 IMAP 操作属于后续阶段。

## 二、产品行为

### 写信窗口

- 主窗口工具栏“新建”变为可用，点击后由 Rust 创建草稿并创建标签为 `composer-<draft-id>` 的独立 Tauri WebView 窗口。
- 窗口包含收件人、抄送/密送展开、主题、富文本编辑器、附件区和发送状态。
- 收件人允许使用逗号或分号分隔；发送时使用严格邮箱地址解析，至少需要一个有效收件人。
- 主题允许为空，但发送前显示确认；正文允许为空。
- 编辑后 800 毫秒自动保存，窗口失焦或关闭前再提交一次；界面明确显示保存中、已保存或失败。
- 已发送草稿转为只读完成状态并关闭窗口；失败时保持窗口和所有内容，允许重试。

### 富文本编辑器

- 首轮支持段落、标题、粗体、斜体、下划线、删除线、项目符号/编号列表、引用、链接、撤销和重做。
- 工具栏、按钮、焦点、禁用和选中状态全部使用现有设计令牌与基础组件。
- 粘贴内容由 ProseMirror schema 约束；发送前 HTML 仍由 Rust 进行邮件输出级过滤和规范化。
- 不在本阶段加入图片上传、表格、代码块、模板变量或商业 Tiptap 扩展。

### 附件

- 前端只能通过系统文件选择器选择文件，不能遍历文件系统。
- Rust 读取用户明确选择的文件，限制单文件和总大小，保存内容哈希、原始文件名、MIME 类型和大小。
- 草稿删除附件只解除引用；内容文件由后续清理任务按引用回收。
- 发件 MIME 使用存储中的附件副本，不依赖原文件继续存在。

## 三、数据模型与恢复

新增嵌入式迁移：

- `drafts`：账户槽、状态、To/Cc/Bcc、主题、编辑器 JSON、HTML、纯文本、修订号、创建/更新时间。
- `draft_attachments`：草稿、附件 ID、文件名、MIME 类型、大小、内容哈希和顺序。
- `send_jobs`：草稿、账户槽、不可变 MIME 哈希、envelope 收件人、状态、尝试次数、下次尝试时间、稳定错误码和时间戳。

状态固定为草稿 `editing | queued | sent`，发件任务 `queued | sending | sent | failed`。创建任务前必须完成草稿保存、附件入库和 MIME 原子写入；数据库事务只引用已存在的内容哈希。启动时将遗留的 `sending` 恢复为 `queued`。

SMTP 成功只标记本地任务已发送。本阶段不猜测或写入远端 Sent 文件夹；第四阶段建立 Sent/Drafts 映射后再补充服务器归档。

## 四、Rust 边界

稳定 DTO 包括 `DraftContent`、`DraftRecipientFields`、`DraftDetail`、`DraftAttachmentSummary`、`SendJobStatus`、`SendJobSummary` 和 `ComposerBootstrap`。

公共命令：

- `open_composer(account_id)`
- `get_composer_bootstrap(draft_id)`
- `save_draft(draft_id, recipients, subject, content, expected_revision)`
- `add_draft_attachments(draft_id, selected_paths)`
- `remove_draft_attachment(draft_id, attachment_id)`
- `queue_draft_send(draft_id)`
- `retry_send_job(send_job_id)`
- `get_send_job(send_job_id)`

命令继续返回稳定 `CommandResult<T>`。错误只返回错误码、本地化参数和是否可重试，不包含密码、完整服务器响应、内部目录或原始 MIME。

`SendWorker` 在应用启动后串行处理当前单账户队列：原子认领任务、从系统凭据库取密码、通过 lettre `send_raw` 发送不可变 MIME、持久化成功/失败/退避状态，并发出只含 ID、状态和修订号的 `send-job-changed` 事件。

## 五、MIME 生成

- 使用 `mail-builder` 生成 RFC 5322/MIME 原始字节，再交给 lettre `send_raw`。
- `From` 来自当前账户身份，envelope 和 header 收件人分别规范化。
- 同时有纯文本和 HTML 时生成 `multipart/alternative`；附件存在时外层为 `multipart/mixed`。
- 主题、显示名、非 ASCII 文件名和地址由库按标准编码；正文使用 UTF-8 与合适的传输编码。
- 生成后解析回读并做结构测试，覆盖中文主题、显示名、HTML/纯文本和 Unicode 文件名附件。

## 六、窗口与权限

- 主窗口不获得创建任意 WebView、文件系统、Shell 或网络权限；它只调用窄范围 `open_composer` 命令。
- 写信窗口使用独立 `composer-*` capability，仅保留 Tauri 核心必要权限和系统文件选择器。
- 文件选择结果只传给 Rust 的附件命令；业务状态不写入 `localStorage`。
- 写信窗口关闭不退出应用，主窗口和后台发件 Worker 保持运行。

## 七、明确不在本阶段

- IMAP Drafts/Sent 上传、服务器端草稿同步、已发送邮件归档。
- 多账户/身份切换、OAuth、POP3。
- 可配置签名库、模板和变量替换；这些在后续“模板与签名”阶段统一实现。
- 内嵌图片、云附件、延迟/撤销发送、请求回执、优先级和加密签名。
- 完整发件箱管理页面；失败任务先在对应写信窗口呈现。

## 八、实施顺序

1. 增加迁移、领域 DTO、Repository 和恢复测试。
2. 实现附件内容存储、MIME Builder 和结构测试。
3. 实现 SMTP Adapter、持久化 SendWorker、事件与失败重试。
4. 增加窄命令、独立窗口和 `composer-*` capability。
5. 接入 Tiptap、自有编辑器工具栏、草稿自动保存、附件和发送状态。
6. 更新总体架构、阶段记录和功能变更记录，完成自动验证后交付手动验收。

## 九、验收标准

- “新建”能打开独立生产级写信窗口，重复打开同一草稿时聚焦原窗口。
- 中英文、系统/浅色/深色主题和强调色在写信窗口生效。
- 键盘可以完成填写、格式化、附件、发送和错误恢复，焦点可见且字段有标签。
- 编辑内容自动保存；关闭窗口并重新打开草稿后 JSON/HTML/纯文本、收件人、主题和附件一致。
- Unicode 主题、正文、地址显示名和附件文件名生成的 MIME 可被 `mail-parser` 正确解析。
- SMTP 成功后任务为 sent；断网/进程中断后任务仍在并可恢复；永久失败可以显式重试。
- 密码、内部路径和完整 SMTP 响应不进入 SQLite、前端持久化、事件、日志或错误详情。
- `cargo fmt`、Rust 测试、Clippy、前端测试、TypeScript/Vite 构建和 Tauri debug 构建通过。
