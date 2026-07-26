# ADR 0005：分平台窗口壳与独立业务窗口

状态：已采纳

日期：2026-07-13

## 背景

NextMail 需要统一的 SaaS 桌面外观和沉浸式侧栏。Windows 系统标题栏会在主内容之外保留不可定制区域；macOS 用户则依赖系统交通灯的位置、行为和辅助功能语义。完全在两个平台伪造相同窗口按钮会损失 macOS 原生体验，也会扩大窗口权限。

## 决策

- Windows 对主窗口和动态 WebView 设置 `decorations: false`，React 标题栏提供拖动、最小化、切换最大化和关闭。
- macOS 保留 `decorations: true`，使用 `titleBarStyle: Overlay`、隐藏系统标题文本，并保留原生交通灯。React 标题栏避让交通灯，仅提供拖动区域。
- 主窗口、`composer-*`、`settings`、`accounts`、`raw-message` 和瞬时 `notification-*` 分别使用 Capability。所有窗口只增加必要的窗口控制或事件权限；`composer-*` 因发送成功后必须绕过关闭拦截而保留 destroy，单例业务窗口关闭时同样只销毁自身 WebView。
- 设置、账户管理和原始邮件分别采用固定标签 `settings`、`accounts`、`raw-message` 的独立单例 WebView。Rust 负责创建或聚焦，前端不获得任意新建窗口权限；原始邮件窗口复用时事件只传账户/邮件 ID，正文仍通过稳定 Command 读取。
- 偏好仍由 Rust 作为持久化事实来源。成功写入后发出只含偏好 DTO 的事件，让各 WebView 失效或更新本地视图。

## 结果

Windows 获得完整可定制标题栏，macOS 保留原生窗口语义，侧栏可以视觉上贯穿标题区域。代价是必须维护平台配置和自绘按钮的键盘/ARIA 行为，并在 Windows、Intel macOS 和 Apple Silicon macOS 分别验证拖动、缩放与关闭生命周期。当前 Windows 配置已进入用户手动验收，macOS 在实际设备或 Runner 执行前保持“未验证”。
