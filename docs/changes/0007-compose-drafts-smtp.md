# 变更 0007：写信、草稿与持久化 SMTP 发件

日期：2026-07-12

## 变更内容

- 主工具栏“新建”接入 Rust 窄命令并创建独立 `composer-*` Tauri 窗口；旁侧草稿下拉可在关闭窗口或重启后继续编辑本地草稿；新增单独 Capability。
- 引入 MIT 开源 Tiptap/ProseMirror，使用 NextMail 自有工具栏和主题令牌；写信模块动态加载，避免增加主窗口首包。
- 新增 `drafts`、`draft_attachments`、`send_jobs` 迁移和 Repository，草稿保存 JSON/HTML/纯文本及乐观修订号。
- 附件经系统选择器授权，Rust 限制单文件 25 MB、总计 100 MB，并将副本按 SHA-256 存入数据目录。
- 使用 `mail-builder` 生成 Unicode multipart MIME；收件人、抄送和密送组成 SMTP envelope，Bcc 不进入消息头。
- MIME 和发件任务先持久化，`SendWorker` 再从系统凭据库取密码并以 lettre `send_raw` 发送；临时错误有限重试，失败支持显式重试，异常退出状态可恢复。
- 增加中英文写信与错误文案、基本身份签名插入、空主题确认和发件状态反馈。

## 安全与边界

- React 不接触 SQLite、系统凭据或 SMTP；事件只携带账户、草稿、任务 ID、状态和修订号。
- 前端不持久化草稿和文件路径；文件路径只作为一次性用户授权参数传给 Rust，不写入数据库。
- 原始 SMTP 响应、密码、内部路径和 MIME 正文不进入前端事件和稳定错误。
- SMTP 成功不自动猜测或修改 IMAP Sent/Drafts 文件夹。

## 自动验证

- Rust 格式检查、24 个 Workspace 测试和全目标 Clippy 通过。
- 前端 5 个测试、TypeScript/Vite 生产构建通过。
- Tauri Windows debug 无 bundle 构建通过。
