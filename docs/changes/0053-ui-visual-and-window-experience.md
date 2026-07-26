# 0053：统一覆盖式滚动条与独立业务窗口体验修正

日期：2026-07-26

状态：已验收。

## 变更

- 删除前端全局“滚动后显示 700ms”的滚动监听和 `.is-scrolling` 状态。应用纵向滚动容器统一隐藏 WebView 原生滚动条并使用覆盖式 `OverlayScrollArea`，滑块位于既有 padding/margin 内且不占主体宽度；默认常驻，只有文件夹列表显式启用 `autoHide`。
- 自绘滑块把透明拖动命中区与可见细滑块分离。邮件列表使用 12px 的内部命中区并向左内收，降低与右侧分栏 ResizeHandle 的误触；视觉宽度和列表内容宽度保持不变。
- 主窗口不再渲染账户管理弹层。账户菜单和无账户空状态通过 `open_account_management_window` 创建或聚焦单例 `accounts` WebView；账户列表、添加、编辑、重新认证、同步间隔、文件夹映射和移除继续复用原有 Query 与表单实现。
- “查看原始邮件”改为 `open_raw_message_window`。单例 `raw-message` WebView 独立调用 `request_raw_message`，重复查看另一封邮件时宿主只发账户/邮件 ID 定向事件，不在事件或主窗口状态中携带原始 EML。
- 新增 `accounts` 与 `raw-message` Capability；普通窗口继续首次居中并由既有 `window-state` 插件恢复各自几何状态。Windows 使用自绘标题栏，macOS 沿用 Overlay 与原生交通灯。
- 设置数据加载层覆盖完整 WebView，避免标题栏侧栏背景先出现。账户名称和邮箱允许完整换行，头像及菜单箭头不再压缩文本。
- 账户管理详情滚动视口为 Select 等表单控件的外扩焦点环预留左侧安全区，并用等量负外边距保持内容基线，避免高亮边框被视口裁切。
- Rust 外观配置默认值与前端启动回退统一由“跟随系统”改为“浅色”；只影响尚无持久化偏好的环境，不迁移或覆盖已有用户选择。
- 同步进度卡移除文件夹完成数/总数；只有同步当前文件夹邮件摘要时显示“正在同步 文件夹 (已同步/总数)”。
- 通知激活在 Rust 核验目标后先为有效目标排入已读更新，再发出主窗口定位事件，避免通知定位只选中但仍保持未读。
- HTML 正文 iframe 外增加 12px 左右宿主留白；邮件文档内部 CSS、作者布局、清洗规则和 sandbox 权限不变。
- 首次实机检查纠正了错误的全局滚动槽：彻底移除 `scrollbar-gutter` 和 `native-scrollbar-stable`，恢复标题栏最小化/最大化/关闭按钮、Checkbox 勾选图标和窗口右侧背景；账户管理、通知文件夹弹层、纯文本正文和 Composer 也迁入统一自绘纵向滚动区。
- 同步更新中英文生产文案、阶段文档、技术参考、架构基线和 ADR 0005。

## 安全与架构边界

- React 仍不创建窗口、不读取 SQLite、文件系统或邮件服务器；所有窗口入口位于 `src/app/api.ts`，账户和邮件 ID 在 Rust/Repository 边界继续核验。
- 原始邮件内容不进入 Tauri Event；`raw-message` 窗口通过既有稳定 Command 读取，Capability 不开放 Shell、任意文件、任意网络或任意建窗。
- 通知点击的已读动作复用现有持久化待办语义，不直接操作 IMAP Session，也不改变通知候选或同步调度。
- 不增加依赖、数据库迁移、Cargo package 或前端持久化；HTML、TLS、凭据和账户槽隔离不变。

## 验证

- `pnpm test`：通过，32 个测试文件、89 项。
- `pnpm build`：通过；仅保留既有主入口与富文本 chunk 大于 500 kB 的非阻断警告。
- `cargo fmt --all -- --check`：通过。
- `cargo test --offline --locked`：通过，120 项 Rust 单元测试与全部 doc test。
- `cargo clippy --offline --locked --all-targets -- -D warnings`：通过。
- `git diff --check`：通过。
- 未运行 Tauri bundle。

## 验收

- 2026-07-26：用户确认第 17 阶段结束，并明确默认浅色修正无需额外实机验收，可在自动验证通过后直接提交。
