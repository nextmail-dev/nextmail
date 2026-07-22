# 0049：受控桌面通知窗口与点击定位

日期：2026-07-22

状态：已于 2026-07-22 通过 Windows 10 22H2+ 与 macOS 12+ 手动验收。

## 变更

- 新增 `NotificationRuntime`，在 Rust 宿主管理 `notification-*` 临时窗口、超时 generation、覆盖更新、层叠上限、最早窗口淘汰和账户移除清理。
- 通知按主窗口所在显示器的物理工作区和 DPI 定位在右下角并向上层叠；屏幕容纳不足时自动降低同时可见数量，窗口完成定位后才显示。
- 覆盖模式复用同一窗口并通过定向事件更新内容、重置超时；旧超时不会关闭已经替换的新邮件。
- 新增按需加载的通知 React 入口，只展示账户身份、发件人和主题；中文、英文、系统/浅色/深色及主题色沿用现有外观事实来源。
- 点击通知会显示、取消最小化并聚焦主窗口。Rust 先验证账户、文件夹和邮件位置；消息失效时降级到文件夹，文件夹失效时降级到该账户的 Inbox 或首个可选文件夹。
- 偏好改变会关闭当前临时通知；移除账户会关闭属于该账户的通知。

## 安全边界

- 新增独立 `notification-*` Capability，只开放 Tauri 事件监听/卸载，不开放文件系统、任意网络、数据库、对话框、Shell、系统 opener 或任意建窗权限。
- Bootstrap、关闭和激活 Command 会校验通知 ID 与调用窗口 label；通知窗口不能读取其他通知。
- DTO 不含正文、预览、HTML、附件、原始 EML、内部路径、凭据、Token、服务器错误或任意 URL。
- 通知窗口不进入 `window-state` 插件，不持久化瞬时内容或几何状态；本批没有新增依赖。
- 这不是系统通知中心集成，不声明通知历史、系统聚合或勿扰模式能力。

## 自动验证

- `pnpm test`：通过，29 个测试文件、76 项测试。
- `pnpm build`：通过；仅保留既有大 chunk 提示。
- `cd src-tauri && cargo fmt --all -- --check`：通过。
- `cd src-tauri && cargo test --offline --locked`：通过，109 项 Rust 测试。
- `cd src-tauri && cargo clippy --offline --locked --all-targets -- -D warnings`：通过。
- `git diff --check`：通过。

## 手动验收

- Windows 10 22H2+ 与 macOS 12+ 均已验证右下角定位、层叠/覆盖、超时、关闭、偏好层级、点击聚焦与定位、失效目标回退、主题和多显示器行为。
- macOS 已额外验证 Spaces、不同缩放显示器、工作区定位和焦点行为。
