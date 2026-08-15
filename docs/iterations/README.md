# NextMail 迭代索引

当前实现事实与工程约定以 [`../project.md`](../project.md) 和代码为准。本目录用于按阶段追溯范围、变更摘要、验证与验收；早期计划中被后续阶段替代的描述不代表当前行为。

| 阶段 | 主题 | 状态 |
| --- | --- | --- |
| [0001](./0001-onboarding.md) | 首次启动与账户验证 | 已验收 |
| [0002](./0002-imap-local-reading.md) | IMAP 本地阅读 | 已验收 |
| [0003](./0003-compose-drafts-smtp.md) | 写信、草稿与 SMTP | 已验收 |
| [0004](./0004-complete-imap-sync.md) | 完整 IMAP 同步语义 | 已验收 |
| [0005](./0005-saas-ui-refactor.md) | 窗口壳与 SaaS UI | 已验收 |
| [0006](./0006-multi-account.md) | 多账户 | 已验收 |
| [0007](./0007-font-and-attachment-experience.md) | 系统字体与附件体验 | 已验收 |
| [0008](./0008-refactor-hardening.md) | 架构与性能硬化 | 已验收 |
| [0009](./0009-templates-and-signatures.md) | 模板与签名 | 已验收 |
| [0010](./0010-mail-reading-and-reply-experience.md) | 阅读保真与回复/转发 | 已验收 |
| [0011](./0011-search-conversations-and-desktop.md) | 搜索、会话与桌面能力 | 搜索范围已验收，其余未排期 |
| [0012](./0012-experience-optimization.md) | 收信、写信与跨平台体验 | Windows 范围已验收 |
| [0013](./0013-signatures-account-management-notifications.md) | 默认签名、账户管理与通知 | 已验收 |
| [0014](./0014-sync-and-composer-experience-corrections.md) | 同步与 Composer 修正 | 已验收 |
| [0015](./0015-header-first-sync-and-logging.md) | 头部优先同步与日志 | 已验收 |
| [0016](./0016-repository-decoupling-and-robustness.md) | 仓库解耦与健壮性 | 已验收 |
| [0017](./0017-ui-visual-and-window-experience.md) | UI 与窗口体验 | 已验收 |
| [0018](./0018-imap-folder-management-and-local-ordering.md) | 文件夹管理与本地排序 | 已验收 |
| [0019](./0019-mail-reading-optimization.md) | 邮件阅读优化 | 已验收 |
| [0020](./0020-rich-text-editor-fidelity.md) | 富文本编辑器保真 | 已验收 |
| [0021](./0021-pre-release-bug-fixes.md) | 首次测试版发布前修正 | 已验收 |
| [0022](./0022-project-readme-and-release-automation.md) | 项目 README 与发布自动化 | 已验收 |
| [0023](./0023-documentation-consolidation.md) | 长期文档收敛 | 已验收 |
| [0024](./0024-release-mail-style-fidelity.md) | 发布态邮件样式保真修复 | 已验收 |
| [0025](./0025-contacts.md) | 联系人完整能力与初版体验 | 已验收 |

以上编号 iteration 作为历史保留。自 2026-08-09 起，每次新的开发计划使用 `YYYY-MM-DD-NN-主题.md` 命名，`NN` 从 `01` 起表示当天实施顺序，不再分配全局阶段编号；范围、非目标、验证结果和用户验收继续写回同一文件。

| 计划 | 主题 | 状态 |
| --- | --- | --- |
| [2026-08-09-01](./2026-08-09-01-reading-and-bulk-actions.md) | 阅读体验与批量操作修正 | 已验收 |
| [2026-08-09-02](./2026-08-09-02-tray-settings-and-auto-update.md) | 托盘、设置分组与自动更新 | 已验收 |
| [2026-08-09-03](./2026-08-09-03-updater-endpoint-fallback.md) | Updater Geo 响应与主备清单 | 已验收 |
| [2026-08-09-04](./2026-08-09-04-updater-manifest-normalization.md) | Updater 清单 URL 规范化 | 已验收 |
| [2026-08-09-05](./2026-08-09-05-ui-dialog-settings-update-window.md) | 模态层、设置选择项与更新窗口 | 已验收 |
| [2026-08-09-06](./2026-08-09-06-segmented-mime-filenames.md) | 分段 MIME 附件文件名兼容 | 已验收 |
| [2026-08-09-07](./2026-08-09-07-app-icon-refresh.md) | 应用图标更新 | 已验收 |
| [2026-08-09-08](./2026-08-09-08-updater-manifest-generation.md) | Updater 清单自行生成与 v0.3.0 重新发布 | 已验收 |
| [2026-08-14-01](./2026-08-14-01-imap-selective-content-and-attachments.md) | IMAP 正文与附件选择性下载 | 已验收 |
| [2026-08-14-02](./2026-08-14-02-folder-dialog-layering.md) | 文件夹对话框层级与关闭清理 | 已验收 |
| [2026-08-14-03](./2026-08-14-03-desktop-interaction-fixes.md) | 桌面交互与实机问题修正 | 已验收 |
| [2026-08-14-04](./2026-08-14-04-imap-streaming-sync-and-inline-parts.md) | IMAP 流式批取与正文引用资源 | 已验收 |
| [2026-08-15-01](./2026-08-15-01-sync-render-performance.md) | 同步期间前端渲染性能修复 | 已通过手动验收（v0.6.6 发布） |
| [2026-08-15-02](./2026-08-15-02-migration-checksum-line-endings.md) | 迁移校验和行尾不一致修复 | 已通过手动验收（v0.6.6 发布） |
| [2026-08-15-03](./2026-08-15-03-qq-bodystructure-nil-parse-failure.md) | QQ 退信邮件 BODYSTRUCTURE 解析失败修复 | 等待手动验收 |
