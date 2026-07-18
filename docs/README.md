# NextMail 文档索引

项目计划和每次功能变动都随代码保存在本目录中。

- `plans/master-plan.md`：产品目标、技术基线与十二阶段总体路线。
- `architecture.md`：当前有效的模块、存储、安全和设计系统边界。
- `iterations/`：每个实施阶段的范围、手动验收与自动验证。
- `iterations/0002-imap-local-reading.md`：第二阶段计划、当前实现结果与待确认差异。
- `iterations/0003-compose-drafts-smtp.md`：第三阶段写信、草稿与 SMTP 发件实施计划与验收。
- `iterations/0004-complete-imap-sync.md`：第四阶段持续同步、离线操作队列、冲突恢复和 Sent/Drafts 映射计划。
- `iterations/0005-saas-ui-refactor.md`：第五阶段跨平台窗口壳、随包字体、三栏界面与独立设置窗口实施和验收。
- `iterations/0006-multi-account.md`：第六阶段多账户生命周期、Supervisor Registry、并发限制和数据隔离计划。
- `iterations/0007-font-and-attachment-experience.md`：第七阶段系统字体试验、附件区域紧凑化和安全下载后打开计划。
- `changes/`：按实施批次记录已经发生的功能和架构变动。
- `changes/0005-message-and-mailbox-encoding.md`：邮件多字符集、IMAP 文件夹名和旧缓存重建说明。
- `changes/0006-rfc2047-robustness.md`：完整 RFC 2047 encoded-word、安全降级和回归语料。
- `changes/0007-compose-drafts-smtp.md`：独立写信窗口、草稿、附件、MIME 与持久化 SMTP 发件。
- `changes/0008-composer-ui-and-reader-fixes.md`：写信窗口生命周期、主界面视觉层级和阅读器修复。
- `changes/0009-mail-list-and-compose-polish.md`：连续邮件列表、侧栏写信组合按钮和编辑器焦点精修。
- `changes/0010-draft-delete-and-list-alignment.md`：草稿二次确认删除与邮件列表顶部间距修复。
- `changes/0011-complete-imap-sync.md`：持续 IMAP 同步、离线操作、安全删除、Sent/Drafts 与界面操作。
- `changes/0012-sync-and-mail-workspace-polish.md`：无感后台同步、可调/可折叠工作区和基础回复转发。
- `changes/0013-composer-send-progress-and-close.md`：发送中央加载弹层与成功后写信窗口关闭修复。
- `changes/0014-reader-spacing-and-phase-4-acceptance.md`：阅读器正文间距修正与第四阶段验收记录。
- `changes/0015-saas-ui-refactor.md`：自绘窗口壳、SaaS 视觉系统、主工作区和设置窗口重构。
- `changes/0016-ui-refactor-regressions.md`：折叠侧栏尺寸、无痕拖拽条和设置白屏/关闭回归修复。
- `changes/0017-settings-window-lifecycle-fix.md`：设置窗口异步创建生命周期修复。
- `changes/0018-sidebar-actions-and-resize-affordance.md`：侧栏收取/设置入口归位及主题自适应拖拽提示。
- `changes/0019-desktop-polish-and-local-send-time.md`：成功通知层级、发件本机时区、连续分栏与紧凑窗口控制区修复。
- `changes/0020-folder-tree-reader-preferences-and-ui-fixes.md`：IMAP 文件夹树、远程图片阅读偏好、HTML 样式保真与界面细节修复。
- `changes/0021-mailbox-tree-scrollbar-and-list-date.md`：文件夹展开交互、邮件列表固定滚动槽与日期显示规则。
- `changes/0022-windows-cjk-typography-calibration.md`：Windows 中文小字号、字重、灰阶对比与平台渲染校准。
- `changes/0023-startup-responsive-progressive-sync.md`：首帧优先启动、后台服务延后、可见启动同步与渐进式本地视图刷新。
- `changes/0024-single-tauri-rust-crate.md`：撤销根 Cargo Workspace，将核心、协议、存储、迁移和唯一构建缓存收敛到 `src-tauri`。
- `changes/0025-multi-account.md`：多账户配置事务、Supervisor Registry、公平发件、数据隔离和账户管理界面。
- `changes/0026-system-font-and-attachment-experience.md`：系统原生字体、紧凑附件区域、安全下载后打开和另存为。
- `changes/0027-theme-color-palette.md`：设置页“主题色”圆点色板、扩展配色和键盘可访问性。
- `adr/`：需要长期保留理由的架构与安全决策；提议状态不代表已经实施。
- `adr/0001-cargo-workspace-boundaries.md`：第二阶段引入 Cargo Workspace 的历史决策，现已被 ADR 0006 取代。
- `adr/0003-durable-send-pipeline.md`：持久化 MIME 与可恢复 SMTP 发件决策。
- `adr/0004-durable-imap-operation-queue.md`：持久化 IMAP 待办、冲突重放与安全 EXPUNGE 降级。
- `adr/0005-platform-window-chrome.md`：Windows 自绘控制与 macOS 原生交通灯覆盖式标题栏的分平台决策。
- `adr/0006-single-tauri-rust-crate.md`：使用单一 `src-tauri` Cargo package，并以内部模块维持 Rust 业务边界。
- `adr/0007-multi-account-runtime-and-credentials.md`：多账户 Supervisor Registry、匿名数据槽隔离和凭据清理事务。
- `third-party-notices.md`：当前随应用分发的独立第三方资源及许可证说明。

当架构或安全决策出现需要长期解释的取舍时，在 `adr/` 中新增 ADR；单纯实现细节不创建 ADR。
