# ADR 0002：HTML 邮件使用 Rust 清洗与 sandbox iframe

状态：已接受

日期：2026-07-12

## 背景

邮件 HTML、内联资源、远程图片和链接均属于不可信输入。直接插入主 React DOM 会把邮件内容放入具备应用 IPC 能力的页面上下文。

## 决策

由 Rust 使用白名单规则清洗 HTML。前端只在不允许 scripts、forms、same-origin 和 top-navigation 的 sandbox iframe 中渲染清洗结果；远程图片默认阻止。链接边界由 ADR 0008 补充，Composer CID 由 ADR 0009 管理，已收邮件本地 CID 图片由 ADR 0011 管理；其他远程资源仍需在启用前形成独立决策。

## 影响

- 邮件内容不能直接调用 NextMail 前端或 Tauri IPC。
- 链接、CID、本地附件和远程图片必须分别遵循对应 ADR 的受控输入与权限校验。
- 部分复杂邮件样式会被降级，以安全性优先于像素级还原。
