# 0048：通知偏好与新邮件候选

日期：2026-07-22

状态：已于 2026-07-22 通过 Windows 10 22H2+ 手动验收。

## 变更

- 新增注入式 `NotificationPreferencesConfigStore` 和原子 `notification-preferences.json`，保存全局开关、层叠/覆盖模式、最多层叠数量、展示时间，以及逐账户和逐文件夹公开 ID 开关。
- 新增稳定通知偏好 DTO、Command 和 `src/app/api.ts` 方法；最多层叠数量限制为 1–10，展示时间限制为 1–60 秒，账户 ID 和重复覆盖项由 application 校验。
- 设置窗口“通知”分类不再是占位：展示全局开关、模式、数量、时间和账户列表；每个账户右侧三点按钮打开可滚动文件夹列表。账户默认开启，Inbox 默认开启，其他文件夹默认关闭。
- 文件夹通知弹窗固定保留纵向滚动槽并始终显示滚动条，列表开始滚动时不再挤压或横向移动左侧内容。
- 数据格式版本 18 为匿名账户槽增加通知同步基线，并把升级前已经成功同步的账户迁移为就绪。新账户首次完整同步、首次发现文件夹和 UIDVALIDITY 重建均不产生历史候选。
- `MailSyncSink::upsert_message` 返回稳定的新位置结果；摘要首次落库且邮件未读时形成候选，正文回填和重复同步不形成候选。候选仅在整次账户同步成功后按全局、账户、文件夹三级偏好过滤，并按账户/文件夹/消息去重。
- 新增最小 `new-mail-candidate` 事件，只携带公开账户、文件夹、消息 ID、首个发件人姓名/地址和主题。可见通知窗口、层叠/覆盖调度和点击定位留给第四批。

## 边界

- 通知偏好不包含正文、预览、凭据、Token、服务器错误、匿名账户槽或内部路径；密码与邮件数据存储边界不变。
- 偏好读取、候选过滤和 Tauri 事件发布都发生在 SQLite 写事务之外，不持有写锁等待窗口或文件 I/O。
- 本批不创建通知 WebView，不修改现有窗口 Capability，不宣称系统通知中心、Windows/macOS 可见通知或点击定位已经实现。

## 自动验证

- `pnpm test`：通过，28 个测试文件、72 项测试。
- `pnpm build`：通过；仅保留既有大 chunk 提示。
- `cd src-tauri && cargo fmt --all -- --check`：通过。
- `cd src-tauri && cargo test --offline --locked`：通过，105 项 Rust 测试。
- `cd src-tauri && cargo clippy --offline --locked --all-targets -- -D warnings`：通过。
- `git diff --check`：通过。
