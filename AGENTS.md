# AGENTS.md

NextMail 是基于 Tauri 2、React 19 / TypeScript 5.8 和 Rust 的本地优先桌面邮件客户端，当前版本 0.4.0。本文件是给 AI 编码代理（Codex 等）的最小必读指令；完整实现事实、开发细则与项目记忆以 [`docs/project.md`](./docs/project.md) 为准，发生冲突时以代码与 `docs/project.md` 为准。

## 仓库结构

双语言单仓库：

- `src/` — 前端（React / TS / Vite / TanStack Query / Tailwind 4 / Radix / Tiptap / CodeMirror）。
- `src-tauri/` — 桌面与 Rust 后端，是仓库内**唯一**的 Rust package。
- `docs/project.md` — 长期技术文档（事实来源）。
- `docs/iterations/` — 每次开发计划的范围、变更摘要、验证与验收；既有阶段编号仅作为历史保留。
- `docs/adr/` — 长期架构/安全决策，按需查阅；当前单一 Tauri Rust package 边界以 ADR 0006 为准。
- `testdata/mail-rendering/` — 邮件保真与恶意内容回归语料，长期保留。

**禁止**在根目录建立 Cargo Workspace、根 `Cargo.toml`、根 `Cargo.lock`、根 `target` 或业务子 crate。

## 常用命令

前端（仓库根目录）：

```bash
pnpm install
pnpm tauri dev      # 桌面联调入口，复用 Rust 增量缓存
pnpm dev            # 仅纯浏览器层，不能替代 Tauri 验收
pnpm test           # vitest run
pnpm build          # tsc && vite build
```

Rust（在 `src-tauri/` 下执行）：

```bash
cargo fmt --all -- --check
cargo test --offline --locked
cargo clippy --offline --locked --all-targets -- -D warnings
```

最终检查：

```bash
git diff --check
```

注意：

- 仓库没有根 Cargo Workspace，**不要**运行 `cargo test --workspace`。
- 日常不运行 Tauri bundle；仅在用户明确要求发布或打包时执行。
- 前端没有 ESLint/Prettier，也没有普通分支或 PR CI。
- Node.js 依赖只用 pnpm；当前不使用 Python，未来需要时只用 uv。
- 纯文档修改至少检查 Markdown 本地链接、陈旧引用和 `git diff --check`，无需运行产品构建或 Tauri bundle。

## 不可破坏的架构边界

### 前端与 IPC

- React 不直接访问 SQLite、邮件服务器、任意文件系统或系统凭据库。
- 业务组件统一通过 `src/app/api.ts` 和稳定 DTO 调用 Tauri Command，不散落裸 `invoke`。
- `src/app/types.ts` 与 Rust DTO 维持 camelCase 序列化契约。
- TanStack Query 管理读取模型；Event 只携带公开 ID、状态、进度或修订并触发精确失效/重读，**不推邮件正文**。
- Query key 使用集中工厂；邮件详情稳定为 `['message', accountId, mailboxId, messageId]`。

### Rust 分层

- `core/` 不依赖 Tauri、SQLx、`tracing` 或具体协议库。
- `application/` 组织账户生命周期、回复/转发、定义变量等纯业务用例。
- `adapters/` / `protocols/` 隔离系统和第三方类型；`storage/` 提供窄 Repository。
- `src/state.rs` 是唯一组合根，具体配置、凭据、协议、Repository、opener 和通知通过 ports 注入。
- `commands/` 保持薄：只做 DTO 接收、用例委托、稳定错误转换和窄事件发布。
- 公开失败统一为 `CommandError { code, params, retryable }`；UI **不得**收到密码、Token、服务器原始响应、内部路径或堆栈。

### 窗口与 Capability

- 每类窗口（`main`、`composer-*`、`settings`、`accounts`、`definition-*`、`message-preview-*`、`raw-message`、`notification-*`）使用独立最小 Capability；前端没有 Shell、数据库、任意网络、任意文件或任意建窗权限。
- `settings`、`accounts`、`raw-message` 是单例；动态窗口按稳定业务目标复用。
- 窗口状态由 Rust 侧 `window-state` 保存；Windows 用 React 自绘控件，macOS 用 Overlay 和原生交通灯。

## 数据与迁移

- SQLx 迁移在 `src-tauri/migrations/`，**只增不改**：已发布迁移不得修改，只能新增。当前到 `0026`。
- 所有账户业务数据按匿名 `account_slot_id` 隔离。
- 多表可见状态用 SQLx 事务；网络、MIME、慢文件 I/O 不持有 SQLite 写锁。
- 内部路径和内容哈希不返回 React。
- 密码及未来 Token 只进入服务名 `com.taurusxin.nextmail` 的系统凭据库。

## 安全边界

放宽以下任何一条都属安全变更，必须补针对性测试，重大取舍新增/修订 ADR：

- TLS 严格验证系统信任链和主机名，**不提供**忽略证书错误选项。
- 明文 IMAP/SMTP 必须由用户明确确认，并在 Rust 边界复验。
- SMTP 测试不得发送邮件；IMAP 测试不得修改邮箱。
- HTML 邮件由 Rust 权威清洗（Ammonia + cssparser），在无 scripts/forms/same-origin/top-navigation/任意文件/任意网络能力的 sandbox iframe 中渲染。
- 远程图片默认由邮件 CSP 阻止；显式允许后仍用 `no-referrer`。CSS `url()`、`@import`、`@font-face`、固定遮罩、动画和变换继续移除。
- CID/`data:image` 只接受经 media type、解码、文件魔数和大小预算验证的 PNG/JPEG/GIF/WebP/BMP。
- 富 HTML 粘贴必须先经 Rust 清洗和选择器作用域限定。
- **日志不得**记录 `CommandError.params`、凭据、Token、邮件正文或服务器原始响应。

## 前端与体验约定

- 业务页面优先组合 `src/components/ui/`；不暴露浏览器默认表单外观。
- 主题使用语义令牌，不在业务组件写死主题相关颜色。
- 深色主题使用 RGB 等值的零色度黑灰表面，主题色只承担主操作、选中、焦点和必要状态，不作为窗口底色。
- 用户主题色只作为源色保存，界面主色按当前亮暗表面校正到至少 4.5:1 对比度；不得让业务组件绕过校正直接写入主色或固定主色前景。
- 新生产文案**同时**提供 `zh-CN` 与 `en-US`，缺失时回退英文。
- UI 不显示调试说明、内部阶段名或临时占位文案。
- 可点击控件统一使用桌面默认指针，不显示手型 pointer；文本选择、窗格缩放和拖动继续保留对应的语义指针。
- 不移除键盘焦点指示：普通操作和列表行使用仅 `focus-visible` 可见的 1px 内描边，编辑表面使用克制的 2px 内反馈；鼠标点击不得显示粗焦点框。
- 紧凑横向工具栏直接保留高频操作，低频操作收入有名称的更多菜单，不用横向滚动条隐藏操作。
- 任何可能产生纵向滚动的容器统一用 `OverlayScrollArea`：6px 滑块绝对覆盖在右侧，仅在容器 hover 或键盘 focus-within 时显示并使用默认指针；不得为滚动条预留 `padding`、gutter 或空白，滑块出现与消失不得改变内容或分割线坐标。
- 文件夹列表是唯一位置例外：展开侧栏的滚动容器可向右延伸并用等量 `content` 右内边距维持圆角列表项宽度，让滑块位于列表项外侧；不得改用 viewport padding。
- 邮件 HTML 与 Composer 原文不进入主 React DOM；保真优化不能越过安全边界。
- 邮件主题适配只处理 Rust 权威清洗后的字符串：在惰性 DOM 中按清洗器允许的 CSS 子集静态计算 cascade、转换颜色并写回 inline style，再交给原有无 scripts / same-origin 的 sandbox iframe；清洗器须保留常见安全表现属性 `bgcolor`、`font[color]`、`hr[color]` 与表格 `bordercolor`。深色下，高亮等带色背景须先通过继续压暗背景来保留作者文字色，再对仍不达标的文字做最小明度校正，边框相对有效背景至少保持 3:1；浅色下只把不透明纯白作者背景映射为 App 阅读面板表面，其他颜色保持原样。不得为读取计算样式放宽 iframe 或把邮件节点挂入主 DOM；作者已声明原生深色适配时跳过深色转换，图片与视频保持原样。
- 联系人、邮件发件人与账户入口共用主题渐变身份头像；文本裁切容器不得同时裁切头像阴影。
- 仅 Windows 显示 WebView 自绘窗口按钮；macOS 和其他带系统装饰的平台使用原生窗口控制。窗口失焦时降低标题与自绘控件强调度，但保留标题栏边界。

## Git 与交付约定

- **未经明确要求，不 commit、push、创建/移动 tag、改远端、签名或发布。**
- 当被要求提交时，直接提交到 `main`，不创建 feature 分支。
- 不用 `reset`/`checkout` 覆盖工作区，不清理无关文件；不得 reset、覆盖或顺手提交用户已有修改，出现重叠先说明。
- 新依赖优先 MIT、Apache-2.0、BSD、ISC；其他许可先确认并按需更新 `docs/third-party-notices.md`。
- 不提前开发当前范围外行为，不自行扩展依赖、安全策略或重大 UI 决策。
- 正式测试与测试语料长期保留；临时探针、凭据、日志、截图、coverage 和临时数据在验证后清理。`dist/` 和 `src-tauri/target/` 是正常增量缓存，默认保留。
- 用户只要明确说“发布”且未指定版本，就以仓库最新有效 `vMAJOR.MINOR.PATCH` tag 为基准自动递增 patch 版本，同步应用版本后提交、推送、创建并推送新 tag；用户指定版本时使用指定版本。`v*` tag 触发三平台 Release 工作流，普通分支 push/PR/手动 dispatch 不触发发布。

## 文档维护

- `docs/project.md` 是新会话唯一必读的长期技术文档。当前能力、技术栈、目录、数据格式、运行语义、限制或开发约定变化时更新本文。
- 每次收到新的开发计划，先在 `docs/iterations/` 建立或更新一份按 `YYYY-MM-DD-NN-主题.md` 命名的计划文档，`NN` 从 `01` 起表示当天实施顺序；写清状态、范围、非目标和验证门禁，不再分配全局阶段编号。既有编号 iteration 作为历史保留，实施结果与验收写回当前计划文档。状态用“规划中 / 实施中 / 等待手动验收 / 已验收 / 未排期”。
- 重大架构/安全取舍新增 ADR；已有决定变化时更新状态和修订说明。
- 不创建会话式交接文档、独立 change 流水账或重复的总体计划——Git 历史承担逐提交细节。

## 工作流

1. 阅读 `docs/project.md`。
2. 执行 `git status --short` 与 `git log -3 --oneline --decorate`，确认 HEAD、远端关系和未提交修改。
3. 阅读任务涉及的源码、配置、迁移和测试；需要历史范围或设计理由时再查 iteration/ADR。
4. 只实施用户明确给出的当前计划，不从“后续设想”自行选择功能。
5. 按风险完成验证（`cargo fmt --check` / `cargo test` / `cargo clippy` / `pnpm test` / `pnpm build` / `git diff --check`），交付结果和必要的实机验收步骤。
