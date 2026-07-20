# 下一实施批次：前端状态与工作区结构收敛

状态：待实施。

本批属于第八阶段剩余的 P1 前端结构工作。它只做无产品行为变化的前端重构和回归测试；完成并通过用户手动验收后，再决定是否实施低优先级清理，之后才进入 POP3 阶段。

## 1. 目标

- 外观偏好只保留 TanStack Query 一个事实来源。
- 写信窗口关闭监听只订阅一次，不随每次输入反复注册。
- 将主工作区的选择状态、Tauri 事件和分栏布局从 `MainShell` 抽成独立 hooks。
- 保持现有窗口、主题、语言、账户、同步和邮件操作行为不变。

## 2. 实施范围

### 2.1 外观偏好单一数据源

- 删除 `src/app/appearance.ts` 的 Zustand Store 及所有 `useAppearanceStore` 调用。
- `get_preferences` 使用统一的 `appearanceQueryKey`。
- 设置写入使用 `useMutation`：`onMutate` 取消查询并乐观写 cache，`onError` 恢复旧值，成功值覆盖 cache。
- `appearance-preferences-changed` 事件桥只负责 `queryClient.setQueryData` 和调用 DOM 主题应用函数。
- 主窗口、设置窗口和写信窗口各自拥有 QueryClient，但都通过 Rust 持久化事件同步，不共享 React 内存。
- 语言、主题和主题色切换失败时 UI 必须回滚。

### 2.2 写信关闭监听稳定化

- 保持当前保存逻辑和关闭拦截语义。
- 将最新 `saveNow` 放入 ref；`onCloseRequested` effect 只依赖稳定的草稿/账户身份，不依赖收件人、主题或正文。
- 组件卸载时可靠调用一次 unlisten。
- 增加测试，确认正文多次变化不会重复注册监听，关闭仍保存脏草稿，空白草稿仍按既有规则处理。

### 2.3 MainShell hooks

在 `src/features/mail/hooks/`（同时满足 `components.json` 的 `@/hooks` 约定时可选择 `src/hooks/`，但全项目必须统一）拆出：

- `useMailboxSelection`：当前账户、文件夹和邮件选择；账户/文件夹变化后的清理与默认选择。
- `useMailRuntimeEvents`：mailbox、message、sync、send-job、pending-operation 事件注册、Query 失效与清理。
- `usePaneLayout`：文件夹栏折叠、两条分栏宽度、窗口 resize 钳制和拖动更新。

要求：

- 通过函数式更新或 ref 消除 resize stale closure。
- Tauri listeners 在稳定依赖下注册，unlisten 完整可靠。
- Query key 继续使用集中工厂。
- `MainShell` 主要负责组合数据和渲染三栏，不再包含底层 resize 数学和事件 wiring。

### 2.4 测试

- 外观写入成功、失败回滚和事件同步测试。
- 写信关闭监听稳定性测试。
- `usePaneLayout` 的最小/最大宽度、折叠、窗口缩放测试。
- `useMailRuntimeEvents` 的事件到 query invalidation 映射测试。
- 保留现有 MessageViewer、MailboxPane、AccountSwitcher、Settings、Toast 等测试。

## 3. 非目标

- 不改变现有 SaaS UI 视觉、布局尺寸或交互文案。
- 不添加 POP3、OAuth、模板、签名、全文搜索、托盘或系统通知。
- 不改变 Tauri Command、Rust DTO、SQLite schema、同步算法或 Capability。
- 不在本批引入 ESLint、Prettier、GitHub Actions或新运行时依赖；工具链属于单独确认的后续清理。
- 不处理主包拆分、列表 memo、存储映射 helper 和 tracing 等低优先级事项。

## 4. 自动验收

```powershell
pnpm test
pnpm build
Push-Location src-tauri
cargo fmt --all -- --check
cargo test --offline --locked
cargo clippy --offline --locked --all-targets -- -D warnings
Pop-Location
git diff --check
```

Rust 未改动时仍运行完整 Rust 检查，确保前端类型与 Tauri 边界没有意外漂移。不执行 Tauri bundle。

## 5. 手动验收

1. 设置窗口连续切换中文/英文、浅色/深色/系统和多个主题色，主窗口与写信窗口立即同步；重启后保持。
2. 写信时连续输入、关闭窗口并重新打开，草稿只保存一次且内容完整；完全空白草稿不残留。
3. 拖动两条分栏、折叠文件夹栏、缩放窗口，宽度钳制与视觉细线和当前版本一致。
4. 后台同步、附件下载、Flags、移动/复制/删除、草稿和发件事件仍及时刷新界面。
5. 多账户切换后不会保留上一账户的文件夹、邮件选择或事件结果。

用户确认通过后记录验收并提交。随后先编写下一批计划，不直接进入 POP3。

## 6. 后续候选顺序

前端结构批次验收后：

1. 单独评估低优先级清理：列表 memo、轮询放宽、重复格式化工具、深色高亮、首帧语言、SQLx 映射、内部错误日志和共享测试夹具。
2. 单独询问是否引入 ESLint/Prettier 和 GitHub Actions；远程仓库、Workflow、Release 均需明确授权。
3. 完成确认过的清理后进入总体计划第九阶段 POP3，并先编写真实服务器兼容门禁计划。
