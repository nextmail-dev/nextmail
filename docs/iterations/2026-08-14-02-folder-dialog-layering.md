# 文件夹对话框层级与关闭清理

状态：已验收

## 目标

修复文件夹移动、重命名等操作对话框关闭后残留遮罩、导致主界面无法交互的问题；确保移动文件夹的目标选择列表显示在对话框内容之上并可正常选择。

## 范围

- 追踪全部文件夹操作共用的 Dialog 状态、Portal 和 Overlay 生命周期。
- 修复关闭、取消、Escape 与提交后的遮罩清理。
- 修复对话框内 Select/Dropdown Portal 的层级关系。
- 在共享组件或共用调用点完成根因修复，并补回归测试。

## 非目标

- 不调整文件夹业务规则、IMAP 操作或文案。
- 不重构无关弹窗和菜单外观。
- 不提交当前尚待验收的 IMAP 选择性下载阶段。

## 验证门禁

- 前端测试覆盖关闭文件夹操作对话框后 Overlay 消失、目标列表位于 Dialog 之上并可选择。
- `pnpm test`。
- `pnpm build`。
- `git diff --check`。
- 手动验收移动、重命名、创建和删除文件夹对话框的关闭与再次打开。

## 实施结果

- 文件夹 ContextMenu 选择操作后先让菜单完成关闭，再在下一事件循环挂载 Dialog，避免两套 Radix modal pointer-event 锁重叠并残留到 `body`。
- 新增统一 `app-floating-content` 层级 210，高于 Dialog overlay/content 的 200/201；Select、DropdownMenu 和 ContextMenu 的 Portal 内容统一使用该层级。
- 回归测试覆盖从右键菜单打开并关闭重命名 Dialog 后恢复交互，以及移动目标 Select 位于 Dialog 之上并可选择。

## 自动验证

- `pnpm exec vitest run src/features/mail/MailboxPane.test.tsx src/styles/base.test.ts`：2 个文件、17 项测试通过。
- `pnpm test`：42 个测试文件、158 项测试通过。
- `pnpm build`：通过；仅保留既有大 chunk 提示。
- `git diff --check`：通过。

## 手动验收重点

1. 分别打开创建、重命名、移动、删除文件夹对话框，使用关闭按钮、取消、Escape 和点击遮罩关闭，确认主界面立即恢复交互。
2. 连续多次打开并关闭不同文件夹操作，确认不残留遮罩或不可点击状态。
3. 在移动文件夹对话框中展开目标列表，确认列表位于弹框之上且每个目标可选。

## 手动验收

- 2026-08-14：Windows 实机验收通过。
