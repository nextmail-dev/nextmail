# 第二十四阶段：发布态邮件样式保真修复

## 状态

已验收（2026-08-02；用户确认修复通过并要求发布 `v0.1.1`）。

## 目标

修复 Windows 与 macOS 发布构建中邮件内联 CSS 被整体阻止的问题，使安装版与调试模式一致地保留经过 Rust 清洗的字体、颜色、背景、图片尺寸和表格布局，同时维持既有邮件隔离、安全清洗、远程图片和外链边界。

## 根因

- 调试模式从 Vite 开发服务器加载，发布模式使用 Tauri 打包的前端资产。
- Tauri 在发布构建时默认向 `style-src` 自动加入 nonce/hash；CSP 规则因此忽略同一指令中的 `'unsafe-inline'`。
- 邮件使用 `srcdoc` iframe，必须继承父页面 CSP。邮件自身的 `<style>`、`style` 属性和阅读器动态主题样式没有发布资产 nonce/hash，因而在发布态被整体阻止。
- 生产前端包仍包含邮件样式注入逻辑，Rust CSS 白名单也保留相关展示属性；没有发现仅在 release 生效的邮件清洗分支，因此不是 Vite/Tailwind 清理或清洗器模式差异。

## 已确认范围

- 在 Tauri 安全配置中只禁用其对 `style-src` 的自动资产 CSP 修改，使项目显式配置的 `style-src 'self' 'unsafe-inline'` 在发布态保持有效。
- 保持 Tauri 对 `script-src` 等其他指令的默认资产保护，不全局关闭 CSP 修改。
- 保持邮件 Rust HTML/CSS 清洗、`sandbox="allow-popups"`、不透明 origin、`no-referrer`、远程图片默认阻止和 Rust 外链复验不变。
- 增加配置契约回归，确保未来不会误删这项发布态邮件渲染要求，也不会把禁用范围扩大到 `style-src` 之外。
- 更新长期项目手册和现有邮件渲染 ADR，记录发布态 CSP 约束与安全理由。

## 非目标

- 不允许邮件脚本、表单、same-origin、顶层导航、任意网络、任意文件或 Tauri IPC。
- 不放宽 Rust CSS 属性、选择器、URL、资源类型或大小预算白名单。
- 不实现远程 CSS、背景图、Web Font、邮件专用 WebView 或新的自定义协议。
- 不新增依赖、数据库迁移、Capability、发布工作流或范围外 UI。
- 不在本地运行 Tauri bundle；用户验收后已单独明确授权提交、推送和通过 `v0.1.1` tag 发布，线上安装产物继续由既有 GitHub Actions 生成。

## 实施顺序

1. 建立本阶段记录并确认现有 CSP、iframe 与清洗边界。
2. 精确调整 Tauri `style-src` 资产修改配置。
3. 增加配置契约测试并更新 ADR/长期项目记忆。
4. 执行前端、Rust、构建和补丁检查；记录仍需执行的安装版实机验收。

## 验证门禁

- 自动测试必须断言 `dangerousDisableAssetCspModification` 仅包含 `style-src`，主 CSP 仍显式包含 `style-src 'self' 'unsafe-inline'`，且 `script-src` 未被加入禁用列表。
- 既有邮件阅读回归必须继续断言 iframe 只有 `allow-popups`、保留 `no-referrer`，邮件文档仍有严格 CSP，远程图片只能经显式授权扩展。
- 运行 `pnpm test`、`pnpm build`、Rust fmt/test/clippy 和 `git diff --check`。
- 不运行 Tauri bundle。最终仍需在 Windows 安装版和 macOS 应用包中用包含行内样式、样式表、响应式图片及复杂表格的邮件实机确认，并检查控制台不再出现邮件内联样式 CSP 拒绝。

## 实施结果

- `src-tauri/tauri.conf.json` 新增指令级 `dangerousDisableAssetCspModification: ["style-src"]`；没有使用全局布尔开关，也没有禁用 `script-src` 或其他资产 CSP 修改。
- Rust 配置契约测试直接读取真实 Tauri 配置，固定唯一禁用项、有效的内联样式策略和 `script-src 'self'`。
- 项目长期安全说明与 ADR 0008 已记录 `srcdoc` CSP 继承、Tauri 发布资产 nonce/hash 冲突、选择该修复的理由及继续有效的隔离边界。
- 锁定版本 Tauri 2.11.5 的实现核对确认：该设置同时跳过构建期 `<style>` nonce 注入和运行时 `style-src` nonce/hash 追加，`script-src` 处理仍保持启用。

## 自动验证

- `pnpm test`：通过，33 个测试文件、106 项测试全部成功；既有 `SafeMailFrame` sandbox、`no-referrer`、远程图片和作者样式顺序回归保持通过。
- `pnpm build`：通过；保留项目已知的两个大于 500 kB chunk 警告。
- `cargo test --offline --locked`：通过，148 项 Rust 测试全部成功。
- `cargo fmt --all -- --check`：通过。
- `cargo clippy --offline --locked --all-targets -- -D warnings`：通过。
- `pnpm tauri build --no-bundle -- --locked`：通过，完成 `nextmail v0.1.1` Windows release profile 与生产资产处理验证，只生成增量应用二进制，没有创建 NSIS/DMG/AppImage 安装包。
- `git diff --check`：通过；只有 Windows 工作区既有 LF/CRLF 提示，没有空白错误。

## 手动验收

1. 分别使用 Windows 安装版和 macOS 应用包打开同一批包含内联 `style`、邮件 `<style>`、指定宽高图片、响应式布局和复杂表格的邮件。
2. 对照调试模式确认字体、文字颜色、背景、图片尺寸、表格宽度/列宽/边框与浅深色表现一致。
3. 确认远程图片仍默认阻止，只有显式允许后才加载；外链仍交给系统且不在应用 WebView 内打开。
4. 检查安装版开发者控制台不再出现针对邮件 `<style>` 或 `style` 属性的 `style-src` CSP 拒绝；若仍有错误，保留完整有效 CSP 和 WebView 版本用于继续定位。

## 验收结论

- 2026-08-02：用户明确确认本阶段验收通过，并要求将修复作为 `0.1.1` 发布。
- 应用、Tauri、Rust package、前端回退版本与 `X-Mailer` 同步升级为 `0.1.1`；`v0.1.1` tag 推送后由既有 tag-only GitHub Actions 构建并公开三平台 Release。

## 迭代变更摘要

- 0061：精确关闭 Tauri 发布资产对 `style-src` 的 nonce/hash 修改，恢复 sandbox `srcdoc` 中经过 Rust 清洗的邮件内联 CSS，并增加配置契约与长期安全文档。
- 0062：记录用户验收通过，统一升级应用版本为 `0.1.1`，并通过 `v0.1.1` tag 进入既有三平台发布流程。
