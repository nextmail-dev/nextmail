# NextMail 开发与验证指南

更新时间：2026-07-20

## 1. 环境

必须安装：

- Node.js 与 pnpm。
- Rust stable，目标平台对应的 Tauri 2 系统依赖。
- Windows：WebView2 与 MSVC 构建工具。
- macOS：Xcode Command Line Tools；打包、签名和公证尚未进入当前阶段。

Node.js 依赖只能使用 pnpm。项目当前不使用 Python；未来若必须引入 Python 工具，依赖与执行环境统一由 uv 管理。

## 2. 安装与运行

在仓库根目录：

```powershell
pnpm install
pnpm tauri dev
```

`pnpm tauri dev` 会调用 Vite 开发服务器并复用 `src-tauri/target` 增量构建缓存。日常修改不要求重复执行 Tauri 完整 bundle。

仅调试浏览器层时可以运行：

```powershell
pnpm dev
```

但浏览器环境无法替代 Tauri Command、Capability、系统凭据库、文件选择器和窗口生命周期的实机验证。

## 3. 自动验证

前端在仓库根目录：

```powershell
pnpm test
pnpm build
```

Rust 必须进入唯一 package：

```powershell
Push-Location src-tauri
cargo fmt --all -- --check
cargo test --offline --locked
cargo clippy --offline --locked --all-targets -- -D warnings
Pop-Location
```

最后检查补丁格式：

```powershell
git diff --check
```

当前没有根 Cargo Workspace，因此不要运行或在文档中新增 `cargo test --workspace`。正常的 `dist/` 与 `src-tauri/target/` 默认保留，以免每次重建依赖；临时探针、测试数据目录、coverage、日志、截图和测试凭据需要清理。

## 4. 渐进式交付约定

每个批次遵循：

1. 先更新或新增当前批次计划，写清范围、非目标和验收标准。
2. 只实现该批次，不提前开发后续产品功能。
3. 功能或架构变化同步更新 `architecture.md`、阶段文档和 `changes/`。
4. 完成适量自动验证，不为测试留下临时代码或数据；正式单元/组件/集成测试保留。
5. 交给用户使用 `pnpm tauri dev` 实机验收。
6. 用户明确确认通过后记录验收结果；只有用户明确要求时才提交 Git。
7. 验收前不自行进入下一批，存在额外产品细节时先向用户确认。

## 5. 代码边界

前端：

- 业务组件通过 `src/app/api.ts` 调用 Command，不散落裸 `invoke`。
- TanStack Query key 使用集中工厂；Event 只失效相符的本地视图。
- 业务页面优先使用 `src/components/ui/`，不要出现浏览器默认表单样式。
- 新文案同时加入 `zh-CN` 与 `en-US`，缺失时回退英文。
- UI 必须是生产文案，不显示调试说明、开发任务或临时占位信息。
- 主题使用语义令牌，不在业务组件写死主题相关颜色。

Rust：

- 只维护 `src-tauri` 单一 crate，不创建根 Workspace 或业务子 crate。
- `core` 不依赖 Tauri、SQLx 和协议库。
- 具体 Adapter 在 `state.rs` 装配，通过 ports 注入 application/runtime。
- Command 保持薄，只做 DTO 接收、用例委托和窄事件发布。
- Repository 不承担回复/转发内容组合等表现层业务规则。
- 第三方 IMAP/SMTP/MIME/SQLx 类型不得进入公开 DTO。
- 密码、Token、服务器原始响应、内部路径和堆栈不得进入 UI 错误。

## 6. 数据库与存储变更

- 迁移只新增文件，不修改已经存在的迁移。
- 数据格式版本与 `.nextmail-data.json` 兼容性需要同步评估。
- 跨表可见状态使用 SQLx 事务；网络和慢文件 I/O 不应持有 SQLite 写锁。
- 邮件与账户数据始终带 `account_slot_id` 约束，新增查询必须验证账户隔离。
- 原始邮件和附件按内容哈希保存；不要把真实内部路径返回前端。
- 数据目录可整体迁移，但账户配置和凭据不属于该目录。

## 7. 协议与安全变更

- TLS 严格验证系统信任链，不提供忽略证书错误选项。
- 明文协议必须要求用户显式确认，并在 Rust 保存/连接边界再次校验。
- SMTP 测试只能连接并认证，不能发送测试邮件。
- IMAP 测试只能登录和读取 Capability，不修改邮箱。
- 邮件 HTML 安全策略的任何放宽需要同步更新 `architecture.md`，必要时新增 ADR 与恶意语料测试。
- 远程图片、CID、本地附件和外部链接必须经过受控资源或校验边界，不能暴露文件路径或任意网络能力。

## 8. 文档维护

- `technical-reference.md`：当前实现事实。
- `architecture.md`：稳定边界和关键设计。
- `plans/master-plan.md`：产品阶段顺序。
- `plans/next-implementation.md`：下一批决策完整的实施计划。
- `iterations/`：各阶段范围、验证与验收。
- `changes/`：每次已经发生的功能/架构变化。
- `adr/`：需要长期保留理由的重大取舍。
- `handoff-claude.txt`：交接给下一位开发代理的可复制提示词。

代码与文档冲突时，先核对代码和最近 change/ADR，再修正文档；不要用历史计划覆盖已经验收的现状。
