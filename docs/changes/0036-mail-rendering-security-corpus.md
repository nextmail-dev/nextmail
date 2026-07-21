# 0036 邮件渲染安全语料与第十阶段边界

日期：2026-07-21

状态：第十阶段第一批已验收；未改变生产阅读行为。

## 变更

- 新增 `testdata/mail-rendering/` 共享语料与机器可读 manifest，覆盖无样式正文、交易表格、响应式营销邮件、原生深色邮件、混合背景表格、普通链接/远程资源和主动恶意内容。语料均为合成内容，不含真实邮件、凭据或个人信息。
- Rust HTML 清洗测试读取完整共享语料，固定当前主动内容边界：严格文档 CSP，移除脚本、事件、表单、嵌入文档、外部样式表、危险 scheme、CSS 网络资源与固定遮罩。
- `SafeMailFrame` 前端测试复用同一语料中的无样式邮件，并继续精确验证空 sandbox token、无 `allow` 权限和 `no-referrer`。
- 新增提议 ADR 0008，选择继续使用不透明 sandbox iframe；后续 CSS 使用解析后重建的安全子集，外链使用账户/消息归属的不透明 ID、离站确认和 Rust 系统打开边界。
- 明确第二批 CSS parser 的许可证门禁：锁文件中已有的 `cssparser 0.37` 为 MPL-2.0，未经用户确认不声明为直接依赖。

## 范围

- 本批只建立正式语料、回归基线和长期安全决策，不保留临时探针。
- 没有改变 HTML 清洗输出、深色主题、远程图片、外链、Capability、IPC 或回复/转发行为。
- 阅读器保真、受控外链和完整回复/转发仍分别留在第十阶段第二至四批。

## 验证

- `pnpm test`：21 个测试文件、45 项测试全部通过。
- `pnpm build`：TypeScript 与 Vite 生产构建通过；主入口仍有大于 500 kB 的既有提示。
- `cargo fmt --all -- --check` 通过。
- `cargo test --offline --locked`：68 项测试全部通过。
- `cargo clippy --offline --locked --all-targets -- -D warnings` 通过。
- `git diff --check` 通过。
- 未执行 Tauri bundle；正常 `dist` 与 `src-tauri/target` 增量缓存不清理。

## 手动验收

- 2026-07-21：用户在 Windows 确认现有阅读器无回归，接受 ADR 0008 的 sandbox/CSS/外链边界，并明确确认第二批可以直接使用 MPL-2.0 的 `cssparser 0.37`。第一批验收通过。
