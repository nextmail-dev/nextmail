# NextMail

NextMail 是基于 Tauri 2、React、TypeScript 和 Rust 的跨平台桌面邮件客户端。

当前已完成第一阶段首次启动闭环，并实现第二阶段的单账户 IMAP 本地阅读核心：本地文件夹/邮件视图、后台只读同步、安全 HTML/纯文本阅读、按需正文、原始 EML 和附件缓存。第二阶段的受控远程资源与附件打开差异记录在实施文档中；邮件发送仍属于后续阶段。

## 开发命令

```powershell
pnpm install
pnpm test
pnpm build
pnpm tauri dev
```

Rust 检查在项目根目录执行：

```powershell
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

项目使用 pnpm 管理 Node.js 依赖。若未来引入 Python 工具，必须使用 uv 管理。

## 文档

- `docs/README.md`：文档入口与维护规则。
- `docs/plans/master-plan.md`：完整产品与渐进式实施计划。
- `docs/architecture.md`：稳定的架构、安全、Rust 拆包和前端设计系统边界。
- `docs/iterations/0001-onboarding.md`：第一阶段范围与验收方式。
- `docs/iterations/0002-imap-local-reading.md`：第二阶段计划、实现结果和验收差异。
- `docs/changes/`：按实施批次保存的功能和架构变更记录。
