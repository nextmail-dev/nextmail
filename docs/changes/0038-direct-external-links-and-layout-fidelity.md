# 0038 系统外链与传统邮件布局保真

日期：2026-07-21

状态：已于 2026-07-21 通过 Windows 手动验收。

## 实机反馈与修正

- 第一版不透明 link ID、自定义协议和离站确认在 Windows WebView2 实机点击后没有产生动作，未通过验收。用户明确要求撤销确认与链接隔离，安全链接默认交给系统浏览器或邮件程序。
- 实机对比还显示固定宽度营销邮件被拉到整个阅读区。检查确认清洗器虽保留了 `<style>`，却删除了元素 `class`、`id` 及 `table`/`td` 的传统布局属性，并额外注入 `padding:16px`、统一字体、`img/table max-width:100%` 等全局规则，导致原邮件的选择器、居中和宽度约束失效。

## 最终实现

- Rust 规范化并直接保留安全的 `http`、`https`、`mailto` `href`，为链接固定设置 `target="_blank"` 与 `rel="noopener noreferrer"`。危险/未知 scheme、相对或本机路径、URL 用户信息、反斜线、控制字符、双向文本控制符及百分号编码控制字符继续移除。
- 邮件 iframe 仅增加 `allow-popups`，使真实用户点击能形成新窗口请求；仍不开放 scripts、forms、same-origin、top-navigation 或 Tauri IPC。`no-referrer`、严格邮件 CSP 和远程图片默认阻止保持不变。
- 主窗口改由 `WebviewWindowBuilder` 从既有平台配置创建。`on_new_window` 对目标再次执行相同 URL 校验，安全目标直接交给系统关联程序，并始终返回 `NewWindowResponse::Deny`，因此不会在 NextMail 内创建或加载外部网页。
- 第一版的 React 确认框、事件订阅、预览/打开 Command、自定义协议、link ID 和运行时链接存储全部移除。前端不接收点击事件，也不获得通用 opener IPC。
- 清洗器现在保留邮件 CSS 所需的 `class`、`id`，以及 `width`、`height`、`cellpadding`、`cellspacing`、`border`、`align`、`valign`、`bgcolor`、`nowrap` 和传统 `font` 属性。全局 16px 内边距、统一字体/行高、任意断词以及图片/表格最大宽度覆盖已移除，固定宽度居中表格继续按原邮件布局生效。
- 迁移 0011 可能已在本批第一次 Windows 运行时应用，因此按 SQLx 迁移不可变规则原样保留。新增迁移 0012 删除其临时 `message_links` 表，把数据格式提升到 12，并再次失效旧 `safe_html`，使本地原始 EML 按最终清洗规则重建。

## 安全与范围

- 本批没有启用邮件脚本、事件处理器、表单、嵌入页面、same-origin、顶层导航、文件 URL、未知协议或 CSS 网络资源。
- `allow-popups` 只允许点击产生宿主可拦截的新窗口请求；宿主始终拒绝 WebView 窗口创建。系统打开前不会信任文档中的 URL，而是由 Rust 再次校验协议与混淆边界。
- 没有新增依赖、前端 Capability、商业/Cloud/Pro 组件，也没有进入第四批回复/转发富文本导入。

## 自动验证

- `pnpm test`：21 个测试文件、46 项测试全部通过；覆盖唯一 `allow-popups` token、无脚本/表单/同源/顶层导航和既有阅读器回归。
- `pnpm build`：TypeScript 与 Vite 生产构建通过；主入口仍有大于 500 kB 的既有提示。
- `cargo fmt --all -- --check` 通过。
- `cargo test --offline --locked`：85 项测试全部通过；覆盖传统表格布局/class 选择器、安全外链、危险 URL、系统打开前复验及 0011→0012 修正迁移。
- `cargo clippy --offline --locked --all-targets -- -D warnings` 通过。
- `git diff --check` 通过。
- 未运行 Tauri bundle；正常 `dist` 与 `src-tauri/target` 增量缓存不清理。

## 手动验收

- 2026-07-21：用户在 Windows 10 22H2+ WebView2 确认功能正常。安全 HTTPS/`mailto:` 由系统默认程序直接处理，NextMail 内不出现确认框或外部网页窗口。
- 同一固定宽度营销邮件的顶部横幅、内容卡片、文字换行、居中和整体宽度已确认恢复，不再按整个阅读区拉伸。
- macOS 尚未执行，不宣称通过。
