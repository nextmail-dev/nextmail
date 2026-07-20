# NextMail

NextMail 是一个以本地数据为中心的跨平台桌面邮件客户端，使用 Tauri 2、React、TypeScript 与 Rust 构建。

项目目前处于早期开发阶段，主要面向 Windows 10 22H2+ 与 macOS 12+。Windows 是当前主要实机验收平台；Linux 不作为深度适配目标。

## 当前能力

- 多个 IMAP/SMTP 密码账户，支持 TLS、STARTTLS 和显式确认的明文连接。
- 首次启动数据目录选择、账户自动发现、真实连接验证和系统凭据库。
- SQLite 离线邮件视图，启动后先展示本地数据，再由后台渐进同步。
- IMAP IDLE/轮询、增量同步、断线退避和手动收取。
- 已读、星标、移动、复制、归档和删除的离线操作队列。
- RFC 2047、MIME 多字符集与 IMAP modified UTF-7 文件夹解析。
- sandbox iframe 安全 HTML 阅读、远程图片控制、原始 EML 和附件按需下载。
- 独立富文本写信窗口、草稿、附件、持久化发件队列、Sent/Drafts 同步。
- 回复、回复全部、转发和多账户切换。
- 全局或账户范围的富文本邮件模板与签名库管理。
- 中文/英文、系统/浅色/深色主题、主题色和跨平台窗口壳。

尚未实现模板变量、场景默认规则与 Composer 插入、HTML 阅读与回复体验增强、全文搜索、会话聚合和桌面通知。POP3、OAuth 与自动更新保留为未排期设想。完整状态见 [当前技术参考](docs/technical-reference.md) 与 [总体计划](docs/plans/master-plan.md)。

## 架构摘要

```text
React UI
   │  Tauri Command / Event（稳定 DTO）
   ▼
Application + Runtime
   ├─ IMAP / SMTP / MIME / HTML Adapters
   ├─ SQLite Repositories + Content Store
   ├─ System Credential Store
   └─ Account Supervisors / Send Worker
```

- React 不直接连接 SQLite、邮件服务器、任意文件系统或系统凭据库。
- 同步先提交 SQLite，再发送只含 ID、状态和修订号的事件让界面刷新。
- 密码只进入 Windows Credential Manager 或 macOS Keychain。
- Rust 保持单一 `src-tauri` Cargo package，以内部模块和 ports 维持边界。
- 邮件 HTML 在 Rust 端清洗，再由无脚本权限的 sandbox iframe 渲染。

详细设计见 [架构基线](docs/architecture.md)。

## 技术栈

- React 19、TypeScript、Vite、TanStack Query、react-i18next。
- Tailwind CSS 4、Radix Primitives、源码归属的 shadcn 风格组件层。
- Tiptap/ProseMirror 富文本编辑器。
- Tauri 2、Tokio、SQLx/SQLite。
- async-imap、lettre、mail-parser、mail-builder、Ammonia、rustls。

## 本地开发

准备 Node.js、pnpm、Rust stable 和对应平台的 Tauri 2 系统依赖。

```powershell
pnpm install
pnpm tauri dev
```

前端验证：

```powershell
pnpm test
pnpm build
```

Rust 验证必须在唯一 crate 中执行：

```powershell
Push-Location src-tauri
cargo fmt --all -- --check
cargo test --offline --locked
cargo clippy --offline --locked --all-targets -- -D warnings
Pop-Location
```

项目使用 pnpm 管理 Node.js 依赖。当前不使用 Python；未来若需要 Python 工具，统一使用 uv。

完整环境、测试和交付约定见 [开发指南](docs/development.md)。

## 文档

- [文档索引](docs/README.md)
- [当前技术参考](docs/technical-reference.md)
- [架构基线](docs/architecture.md)
- [开发与验证指南](docs/development.md)
- [总体实施计划](docs/plans/master-plan.md)
- [阶段实施记录](docs/iterations/)
- [架构决策记录](docs/adr/)
- [第三方资源与许可证](docs/third-party-notices.md)

## 开发方式

NextMail 采用渐进式实施：每个阶段在 `docs/iterations/` 写明范围和当前状态，完成自动验证后由维护者进行桌面实机验收，确认后才提交并规划后续阶段。功能、架构和安全变化必须与代码一起记录在 `docs/`。
