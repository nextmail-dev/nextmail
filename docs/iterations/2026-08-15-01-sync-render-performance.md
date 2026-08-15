# 同步期间前端渲染性能修复

状态：等待手动验收

## 目标

修复大型邮箱一次性同步大量邮件时主窗口交互（拖拽分隔条等）出现卡顿、不跟手的问题。同步进度与逐封到达事件的更新频率不应阻塞主线程；拖拽分隔条时的重渲染范围应收敛到布局壳层。

## 背景与诊断

`0.6.5` 将 `sync-progress` 事件从逐封触发进度查询 IPC 改为事件载荷直接写入 TanStack Query 缓存，解决了查询风暴与 OOM。但订阅 `['sync-progress', accountId]` 的 `progressQuery` 位于 `MainShell`——主窗口整棵组件树的顶层：每封邮件落库发布一次事件就整树重渲染一次（含 50 行邮件列表与阅读区），大批量同步时达到每秒数十次，主线程被占满，拖拽分隔条的 pointermove 更新无法及时渲染。

同时 `ResizeHandle` 每次指针移动都会更新 `MainShell` 布局状态，整树无 memo 隔离，列表、阅读区、联系人工作区全部跟着重渲染。

## 范围

- `useMailRuntimeEvents` 将 `sync-progress` 事件按账户缓冲、约 100ms 合并刷写一次缓存，保留 revision 拒绝迟到事件；突发期间中间值可丢弃，最终值仍准确。
- `message-arrived` 缓冲扩展到全部邮箱：当前选中邮箱在刷写时合并插入首页缓存，其余邮箱合并为每 100ms 一次失效，不再逐封失效。
- 同步进度查询订阅从 `MainShell` 下沉到 memo 化的侧栏包装组件；`MailboxPane` 对外 props 契约不变，同步期间只有侧栏子树按合并频率重渲染。
- `MessageListPane`、`MessageViewer`、`ContactsWorkspace`、`AccountSwitcher` 使用 `React.memo`；`MainShell` 与 `useMailboxActions` 的回调与空数组兜底全部稳定化，使拖拽分隔条时这些子树不重渲染。
- 更新 `docs/project.md` 同步模型中进度事件合并刷写的表述。

## 非目标

- 不改变事件载荷、DTO、Query key 或 Rust 侧发布频率。
- 不引入列表虚拟化；不改 IMAP 同步、存储或会话预算。
- 不改 `MailboxPane` 组件 props 及其测试。

## 验证门禁

- `useMailRuntimeEvents` 测试覆盖 1000 条 `sync-progress` 突发只产生一次缓存写入且最终值正确、迟到 revision 被拒绝；非选中邮箱的到达事件合并为单次失效。
- `pnpm test`。
- `pnpm build`。
- `git diff --check`。
- Windows 实机：大型邮箱同步进行时拖拽文件夹/邮件列表分隔条跟手，进度与逐封可见行为不变。

## 实施结果

- `useMailRuntimeEvents` 中 `sync-progress` 事件按账户缓冲最新载荷、约 100ms 合并一次 `setQueryData`，revision 守卫保留在刷写处；1000 条突发事件只产生一次缓存写入，最终值仍为最高 revision。
- `message-arrived` 缓冲扩展到全部邮箱：刷写时当前选中邮箱合并插入首页缓存，其余邮箱每 100ms 合并为一次失效，不再逐封失效。
- `MainShell` 不再订阅同步进度查询；新增 memo 化的 `SidebarPane` 包装组件持有该订阅并渲染 `MailboxPane`（props 契约不变），同步期间只有侧栏子树按合并频率重渲染。
- `MessageListPane`、`MessageViewer`、`ContactsWorkspace`、`AccountSwitcher` 导出改为 `React.memo`；`MainShell` 的窗格回调全部 `useCallback` 化，`useMailboxActions` 返回稳定回调，空数组兜底改为模块级常量，拖拽分隔条时这些子树不再重渲染。

## 自动验证

- `pnpm test`：43 个测试文件、162 项测试通过（含更新后的 1000 条 `sync-progress` 突发合并为一次缓存写入、迟到 revision 拒绝，及非选中邮箱到达事件合并失效断言）。
- `pnpm build`（含 `tsc`）：通过；仅保留既有大 chunk 提示。
- `git diff --check`：通过。

## 手动验收重点

1. 用大型邮箱开始同步，同步持续期间来回拖拽文件夹分隔条与邮件列表分隔条，确认跟手无明显延迟。
2. 同步期间邮件仍逐封（100ms 合并）出现在当前文件夹列表，进度条持续推进，不出现整窗卡死或内存持续增长。
3. 同步期间打开邮件、标记已读/星标、移动邮件，确认交互路径不受影响。
4. 非同步状态下拖拽分隔条、切换文件夹、切换账户、进入联系人工作区，确认行为与之前一致。
