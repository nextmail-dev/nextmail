# Windows WebView 退出清理

状态：已验收

## 目标

- 退出 App 时先显式销毁全部 WebView 窗口，再请求 Tauri 退出，避免 Chromium 在 Windows 上报告 `Failed to unregister class Chrome_WidgetWin_0. Error = 1412`。

## 范围

- 将前端退出命令、托盘退出和主窗口“退出”动作统一经过同一个 Rust helper。
- 销毁当前所有 WebView 窗口后调用既有 `app.exit(0)`。
- 仅修复明确退出路径，不改变最小化到托盘、普通子窗口关闭或平台窗口行为。

## 非目标

- 不用 `std::process::exit(0)` 绕过 Tauri 的 `RunEvent::Exit` 清理。
- 不升级 Tauri、WebView2 或新增依赖。

## 验证门禁

- `cargo fmt --all -- --check`、相关 Rust 测试、`cargo clippy --offline --locked --all-targets -- -D warnings` 与 `git diff --check` 通过。
- Windows 实机从主窗口确认退出和托盘菜单退出，进程均正常结束且控制台不再出现错误 1412。

## 实施结果

- 新增单一 `exit_app` 出口，枚举并显式 `destroy()` 全部 WebView 窗口，随后调用 `app.exit(0)`。
- 前端退出命令、托盘退出和主窗口确认退出均改用该出口；最小化到托盘不受影响。
- 保留 Tauri `RunEvent::Exit`，window-state 等插件仍能执行退出清理。

## 验证结果

- `cargo fmt --all -- --check`：通过。
- `cargo test --offline --locked`：170 项测试通过。
- `cargo clippy --offline --locked --all-targets -- -D warnings`：通过。
- 未执行 Tauri bundle；错误是否消失需通过 Windows 实机退出验证。

## 手动验收

- 2026-08-13：Windows 实机验收通过，App 正常退出且不再出现 Chromium `Chrome_WidgetWin_0` 错误 1412。
- 本阶段变更随 `v0.4.1` 发布。
