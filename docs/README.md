# NextMail 文档索引

项目计划和每次功能变动都随代码保存在本目录中。

- `plans/master-plan.md`：产品目标、技术基线与十阶段总体路线。
- `architecture.md`：当前有效的模块、存储、安全和设计系统边界。
- `iterations/`：每个实施阶段的范围、手动验收与自动验证。
- `iterations/0002-imap-local-reading.md`：第二阶段计划、当前实现结果与待确认差异。
- `iterations/0003-compose-drafts-smtp.md`：第三阶段写信、草稿与 SMTP 发件实施计划与验收。
- `iterations/0004-complete-imap-sync.md`：第四阶段持续同步、离线操作队列、冲突恢复和 Sent/Drafts 映射计划。
- `changes/`：按实施批次记录已经发生的功能和架构变动。
- `changes/0005-message-and-mailbox-encoding.md`：邮件多字符集、IMAP 文件夹名和旧缓存重建说明。
- `changes/0006-rfc2047-robustness.md`：完整 RFC 2047 encoded-word、安全降级和回归语料。
- `changes/0007-compose-drafts-smtp.md`：独立写信窗口、草稿、附件、MIME 与持久化 SMTP 发件。
- `changes/0008-composer-ui-and-reader-fixes.md`：写信窗口生命周期、主界面视觉层级和阅读器修复。
- `changes/0009-mail-list-and-compose-polish.md`：连续邮件列表、侧栏写信组合按钮和编辑器焦点精修。
- `changes/0010-draft-delete-and-list-alignment.md`：草稿二次确认删除与邮件列表顶部间距修复。
- `adr/`：需要长期保留理由的架构与安全决策；提议状态不代表已经实施。
- `adr/0003-durable-send-pipeline.md`：持久化 MIME 与可恢复 SMTP 发件决策。

当架构或安全决策出现需要长期解释的取舍时，在 `adr/` 中新增 ADR；单纯实现细节不创建 ADR。
