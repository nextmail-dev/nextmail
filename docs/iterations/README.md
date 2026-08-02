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

新增阶段时先创建连续编号的 iteration，记录状态、范围、非目标和验证门禁；实施批次、验证结果和用户验收继续追加到同一文件，不再新建独立 change 文档。
