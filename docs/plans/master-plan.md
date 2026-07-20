# NextMail 总体实施计划

## 产品目标

NextMail 是面向 Windows 10 22H2+ x64 和 macOS 12+ Intel/Apple Silicon 的桌面多协议邮件客户端。技术栈为 Tauri 2、React/TypeScript 和 Rust；Linux 不作为深度适配或验收平台。

核心能力包括加密或非加密的 POP3、IMAP、SMTP，多账户和密码/OAuth 认证，邮件与签名模板，富文本写信，中英文及可扩展语言包。

## 架构基线

- React 不连接 SQLite 或邮件服务器；查询本地数据视图，写操作通过 Rust 业务命令完成。
- 同步先落本地数据库，再发送只含 ID 和修订号的事件通知前端失效查询。
- 邮件协议、存储、凭据、安全渲染、后台 Worker 和系统集成都在 Rust 侧。
- 领域与应用层不暴露 SQLx、协议库或 Tauri 类型。
- Rust 只使用 `src-tauri` 下的单一 Cargo package；`core`、`storage`、`protocols` 等职责通过内部模块隔离，仓库根目录不再维护 Cargo Workspace、lockfile 或 `target`。
- 功能依赖优先使用 MIT、Apache、BSD、ISC 等宽松许可证，其他许可证需单独确认。

前端基线为 React 19、TypeScript、Vite、TanStack Query、Zustand、react-i18next、Tailwind CSS 和基于 shadcn 结构自建的组件层。交互原语采用 Radix Primitives，编辑器使用开源 Tiptap/ProseMirror。

Rust 基线为 Tauri 2、Tokio、serde、tracing；协议适配计划采用 async-imap、lettre 和隔离的 POP3 Adapter；MIME 使用 mail-parser/mail-builder；本地数据采用 SQLite WAL、FTS5 和 SQLx 嵌入式迁移；凭据进入 Windows Credential Manager/macOS Keychain。

## 数据与安全边界

可迁移数据目录包含 `content.sqlite`、`raw/`、`attachments/`、`cache/` 和 `.nextmail-data.json`。账户地址、服务器配置、账户到匿名数据槽映射、本机偏好及当前数据目录路径位于系统应用配置区；密码和 OAuth Token 只进入系统凭据库。

HTML 邮件由 Rust 白名单清洗，邮件资源重写为不透明 ID，并在无 scripts、forms、same-origin 和 top-navigation 权限的 sandbox iframe 中显示。远程图片默认阻止；外部链接经 Rust 校验后交给系统默认程序。

## 渐进式实施顺序

1. 工程与首次启动闭环：设计系统、语言、数据目录、真实 IMAP/SMTP 密码认证、凭据保存和主界面空壳。
2. 单账户 IMAP 本地阅读：SQLite 邮件模型、文件夹、默认 90 天同步、安全正文、附件按需下载和离线读取。
3. 写信、草稿与 SMTP 发件：独立窗口、Tiptap、自有工具栏、附件、基本签名、MIME 和持久化发件箱。（已验收）
4. 完整 IMAP 同步语义：IDLE/轮询、Flags、移动/复制/删除/归档、离线操作队列、冲突恢复，以及基础回复/回复全部/转发。（已验收）
5. 跨平台窗口壳与 SaaS UI 重构：Windows 自绘窗口控制、macOS 原生交通灯覆盖式标题栏、随包字体、沉浸式侧栏、邮件列表/阅读器重构和独立设置窗口。（已验收）
6. 多账户：账户管理、Supervisor Registry、并发限制、独立同步策略和账户层级导航。（已验收）
7. 系统字体与附件体验优化：改用 Windows/macOS 原生字体栈，压缩阅读器附件区域，并补齐安全的按需下载、另存为与系统打开闭环。（已验收）
8. 架构与性能重构：分批处理正确性、同步/存储热路径、Rust 分层、前端状态与工具链；P0 与 P1 Rust 分层均已验收，下一批为 `plans/next-implementation.md` 定义的 P1 前端结构。
9. POP3：TLS/STARTTLS/非加密、UIDL、服务器副本策略和真实服务商兼容门禁。
10. 模板与签名：作用域、变量替换、可可靠替换的签名节点，以及新建/回复/转发场景模板；基础回复与转发已在第四阶段提供。
11. Google 与 Microsoft OAuth：系统浏览器、PKCE、回调、刷新、撤销和重新认证。
12. 搜索、会话与桌面集成：FTS5、会话聚合、托盘、未读数、通知和窗口行为。
13. 迁移、发布与硬化：跨机重绑定、性能和损坏恢复、安全审计、签名、公证、发布与自动更新。

每个阶段先形成独立实施与验收文档；完成自动验证后由用户手动验收，只有确认通过才规划和实施下一阶段。

## 当前非目标

统一收件箱、联系人簿、邮件规则、延迟或撤销发送、EML/MBOX 导入导出、日历、PGP/S-MIME、企业策略、遥测以及 Linux 深度适配不在当前标准基线中。

## 执行约束

- 功能变动同步更新架构、阶段计划和变更记录；重大架构或安全取舍增加 ADR。
- Node.js 依赖只用 pnpm；未来若需要 Python 工具，只用 uv。
- 正式单元/组件/集成测试长期保留；临时探针、目录、凭据、日志、截图和 coverage 在验证后清理。`dist`、`src-tauri/target` 等正常构建缓存默认保留，以支持增量构建。
- 日常迭代不再重复执行 Tauri 完整构建；自动验证运行 Rust 测试/Clippy 与前端测试/构建，用户通过 `pnpm tauri dev` 完成桌面联调。发布或明确要求时再执行 Tauri bundle 构建。
- 不提前实施当前确认阶段之外的功能，不初始化或发布远程仓库。
