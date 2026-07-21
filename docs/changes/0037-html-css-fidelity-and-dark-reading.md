# 0037 HTML/CSS 保真与深色阅读

日期：2026-07-21

状态：第十阶段第二批已验收。

## 变更

- Rust HTML 清洗不再整段删除 `<style>`。新增独立 CSS 安全模块，使用 `cssparser 0.37` 对内嵌样式表和行内声明进行语法解析、token 检查和重新序列化，再把结果交给既有严格 CSP 与 sandbox iframe。
- 保留邮件常用的颜色、字体、表格、边框、尺寸、间距、Flex、背景渐变和普通/属性选择器；保留受控 `min/max-width`、方向与 `prefers-color-scheme` 媒体查询。
- 继续丢弃外部样式表、`@import`、`@font-face`、其他 at-rule、网络 `url()`、未知函数、固定/粘性定位、层级、动画、变换和其他未进入白名单的能力。CSS 资源不会绕过远程图片默认阻止。
- 为样式表、行内样式、选择器、声明值、规则数和嵌套深度设置上限；CSS 解码后重新序列化，并阻止转义后的 `</style>` raw-text 逃逸。
- 浅色/深色阅读兜底移除 `!important`，并把有效 `color-scheme` 设置到 iframe 元素和内部文档。无明确样式的邮件继续获得白底深字或深灰底浅灰字；作者在 `html/body`、类或行内明确设置的颜色和背景按正常层叠优先。完整 HTML 的 `<body style>` 会转换为安全的内部正文容器，避免 fragment 清洗丢失页面级行内配色。
- 新增数据格式版本 10。迁移只失效包含 `safe_html` 的旧正文缓存并把对应邮件标记为待重新获取；纯文本正文记录保持不变。正文请求会先按账户槽读取本地原始 EML，在 blocking worker 中重新解析/清洗并事务写回，只有本地原文缺失或不可解析时才访问 IMAP。

## 依赖与边界

- 用户已在第一批验收时明确确认直接使用 MPL-2.0 的 `cssparser 0.37`。该 crate 此前已由 Ammonia 间接存在于锁文件，本批关闭其默认 features，没有新增传递依赖。
- `sandbox=""`、`no-referrer`、文档 `default-src 'none'` 和远程图片显式授权边界不变。
- 本批没有实现外链、通用网络资源、CID、Capability、IPC、回复/转发或 Composer schema 变化。

## 验证

- `pnpm test`：21 个测试文件、46 项测试全部通过。
- `pnpm build`：TypeScript 与 Vite 生产构建通过；主入口仍有大于 500 kB 的既有提示。
- `cargo fmt --all -- --check` 通过。
- `cargo test --offline --locked`：79 项测试全部通过。
- `cargo clippy --offline --locked --all-targets -- -D warnings` 通过。
- `git diff --check` 通过。
- 未运行 Tauri bundle；正常 `dist` 与 `src-tauri/target` 增量缓存不清理。

## 手动验收

- 2026-07-21：用户在 Windows 确认功能正常，第二批验收通过。
