# 0031 前端状态与工作区结构收敛

日期：2026-07-20

## 变更

- 删除外观 Zustand Store 和 `zustand` 依赖；主窗口、设置窗口与写信窗口统一使用 `appearanceQueryKey` 和各自 WebView 的 TanStack QueryClient。
- 外观写入改为 mutation：写入前取消查询并乐观更新 cache，失败恢复旧值，成功以 Rust 返回值覆盖；偏好事件桥只更新 cache 和 DOM 主题。
- Composer 关闭监听改为按账户/草稿身份单次订阅，通过 ref 读取最新保存函数、脏状态和发件状态，避免输入时重复注册。
- 新增 `useMailboxSelection`、`useMailRuntimeEvents` 和 `usePaneLayout`，分别承载主工作区选择、运行时事件和分栏尺寸；`MainShell` 保留数据与三栏组合。
- 邮件账户、文件夹、列表、详情、草稿、同步进度和待办状态 Query key 收敛到集中工厂。
- 新增外观乐观写入/失败回滚/事件同步、Composer 关闭、选择清理、事件失效和分栏钳制/折叠/窗口缩放测试。
- 更新技术参考、架构、总体计划和阶段文档；后续范围与进度统一写入 `iterations/`，不再维护独立的 `next-implementation.md` 或交接提示文件。

## 保持不变

- Tauri Command、Rust DTO、SQLite schema、同步算法、Capability、窗口种类、产品文案和三栏视觉尺寸均未改变。
- 各 WebView 仍只通过 Rust 持久化偏好和窄事件同步，不共享 React 内存；未放宽文件、网络、凭据或 HTML 邮件安全边界。
- 未添加 POP3、OAuth、模板、签名、搜索、通知、工具链或远程仓库配置；未引入新运行时依赖。

## 验证

- `pnpm test`：19 个测试文件、39 项测试全部通过。
- `pnpm build`：TypeScript 与 Vite 生产构建通过；主入口仍有大于 500 kB 的既有提示，留待后续独立批次。
- `cargo fmt --all -- --check` 通过。
- `cargo test --offline --locked`：58 项测试全部通过。
- `cargo clippy --offline --locked --all-targets -- -D warnings` 通过。
- `git diff --check` 通过。

未执行 Tauri bundle，正常 `dist` 与 `src-tauri/target` 增量缓存继续保留。

## 手动验收

2026-07-20：用户完成 Windows 实机验收并确认通过。
