# 0040 当前文件夹本地全文搜索

日期：2026-07-22

状态：已于 2026-07-22 通过 Windows 手动验收。

## 变更

- 第十一阶段细化为四个可独立验收的批次；第一批沿用现有“搜索当前文件夹”入口，跨账户搜索与统一收件箱不进入范围。
- 数据格式版本 15 新增常规存储的 SQLite FTS5 `message_search` 表，迁移会回填既有主题、地址、预览、纯文本正文和附件名。
- 邮件头/预览、正文和附件触发器在既有 SQLite 事务内维护索引。HTML、原始 EML、凭据、路径和服务器错误不进入语料。
- 新增薄 `search_messages` Command 与 `src/app/api.ts` 封装，返回既有 `MessageListPage`。Repository 同时约束匿名账户槽、文件夹、可见位置和日期游标。
- 三字及以上查询使用字面 trigram FTS，中文一至两个字符使用受限字面扫描；用户输入不会被解释为 SQL 或 FTS 操作符。
- 前端搜索增加 250ms 防抖和包含搜索文本的 Query key，移除只筛选已加载列表项的内存逻辑。正文、收件人或附件名命中的服务端结果会完整显示。

## 保持不变

- 普通邮件列表、详情 DTO、同步事件和分页排序不变；清空搜索恢复原列表。
- React 不访问 SQLite；事件仍只失效 `messages` 查询前缀，不推送正文或搜索结果。
- 未实现会话聚合、跨账户搜索、统一收件箱、托盘、系统通知、POP3、OAuth 或全局优化。
- 未运行 Tauri bundle，未提交。

## 验证

- 前端：24 个测试文件、54 项测试通过；`pnpm build` 通过，保留既有大 chunk 提示。
- Rust：`cargo fmt --all -- --check`、98 项 `cargo test --offline --locked` 和严格 Clippy 通过。
- `git diff --check` 在交付前通过。
- Windows 手动验收已确认通过；macOS 未执行。
