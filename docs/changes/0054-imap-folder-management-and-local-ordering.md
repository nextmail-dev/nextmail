# 0054：IMAP 文件夹管理与本地排序

## 状态

已于 2026-07-27 通过手动验收。

## 变更

- 新增文件夹右键菜单：
  - 新建子文件夹；
  - 重命名；
  - 移动到顶层或另一个文件夹下；
  - 删除；
  - 全部标为已读。
- “邮件文件夹”标题支持右键新建顶层文件夹。
- 新增文件夹输入、目标选择和删除确认弹层；`INBOX` 的重命名、移动和删除在前后端均受保护，不可选择文件夹不执行全部已读。
- 展开文件夹栏支持长按 360ms 后拖拽：
  - 只接受同一父级目标；
  - 根据目标上/下半区决定插入位置；
  - 父文件夹携带完整下级子树；
  - 跨层拖拽不改变服务器层级。
- 指针捕获只在 360ms 长按成立后启用，普通单击不会被拖拽行截获，文件夹名称与展开按钮保持原有点击行为。
- 新增 `useMailboxActions` TanStack mutation 与 `useMailboxReorderGesture`，文件夹 Query 仍是视图唯一数据源；成功操作通过最小事件和 Query 失效刷新。
- 新增稳定 IPC：`create_mailbox`、`rename_mailbox`、`move_mailbox`、`delete_mailbox`、`mark_mailbox_all_read`、`reorder_mailboxes`；前端不接触 IMAP 路径或 modified UTF-7。
- `ImapSyncProvider` 新增稳定文件夹操作 port；async-imap Adapter 复用账户第三条交互会话执行 `CREATE`、`RENAME`、`DELETE` 与批量 `UID STORE`，并在 Adapter 内编码 Unicode 叶名称。
- Adapter 新增账户级 mailbox 路径读写锁：同步、正文、消息待办和 APPEND/草稿替换仍可共享并发；结构操作与全部已读等待旧路径/Flags 操作完成，且不持有 SQLite 锁等待网络。
- 新增 `MailboxRepository`：
  - 按 `account_slot_id` 解析结构操作上下文；
  - 服务器成功后事务创建/删除本地投影；
  - `RENAME` 保留 mailbox ID 并同步改写全部下级路径；
  - 批量清除本地未读状态；
  - 校验并保存完整账户文件夹顺序。
- 数据格式升级到版本 20；迁移 0020 为 `mailboxes` 增加可空 `local_sort_order`。未拖拽账户保持既有角色/名称顺序，显式排序后持久化完整顺序，新发现文件夹追加在后。
- 新增 ADR 0015，固定“文件夹结构在线确认”和“本地顺序不改变服务器层级”的长期边界。
- 中英文生产文案、modified UTF-7、路径校验、重命名树、排序集合、子树拖拽和右键菜单回归已补齐。

## 验证

- `pnpm test`
- `pnpm build`
- `cargo fmt --all -- --check`
- `cargo test --offline --locked`
- `cargo clippy --offline --locked --all-targets -- -D warnings`
- `git diff --check`

## 验收关注

- 不同服务商对非空文件夹、含下级文件夹 DELETE 的拒绝语义应只显示错误，不提前删除本地投影。
- 重命名或移动带下级的父文件夹后，下一次完整同步不得生成旧/新路径重复项。
- 本地拖拽顺序在重启和完整同步后保持，且 Web 邮箱顺序/层级不受影响。
