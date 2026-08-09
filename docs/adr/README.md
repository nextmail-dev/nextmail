# NextMail 架构决策索引

新会话只需完整阅读 [`../project.md`](../project.md)。遇到相关架构或安全边界需要理解理由时，再按主题查阅本目录。

- [0001：第二阶段引入 Cargo Workspace（已被 0006 取代）](./0001-cargo-workspace-boundaries.md)
- [0002：HTML 邮件使用 Rust 清洗与 sandbox iframe](./0002-safe-mail-rendering.md)
- [0003：持久化 MIME 发件管线](./0003-durable-send-pipeline.md)
- [0004：持久化 IMAP 操作队列与安全删除降级](./0004-durable-imap-operation-queue.md)
- [0005：分平台窗口壳与独立业务窗口](./0005-platform-window-chrome.md)
- [0006：Rust 收敛为单一 Tauri package](./0006-single-tauri-rust-crate.md)
- [0007：多账户运行时与凭据事务](./0007-multi-account-runtime-and-credentials.md)
- [0008：邮件渲染保真度与交互边界](./0008-mail-rendering-fidelity-boundary.md)
- [0009：Composer 原始 HTML 与内嵌图片边界](./0009-composer-source-html-and-inline-images.md)
- [0010：本地邮件全文搜索边界](./0010-local-message-search.md)
- [0011：已收邮件本地内联图片边界](./0011-inline-cid-reading.md)
- [0012：受控桌面通知窗口与定位边界](./0012-controlled-desktop-notification-windows.md)
- [0013：显式账户同步调度](./0013-explicit-account-sync-scheduling.md)
- [0014：账户级有界 IMAP 会话协调](./0014-bounded-account-imap-sessions.md)
- [0015：在线 IMAP 文件夹结构操作与独立本地顺序](./0015-online-imap-folder-mutations-and-local-order.md)
- [0016：签名更新与区域化传输地址](./0016-signed-updates-and-regional-delivery.md)

ADR 0001 仅作为被取代的历史决策保留，当前 Rust 边界以 0006 为准。ADR 0007 的凭据事务和匿名数据槽仍有效；同步调度以 0013 为准，会话并发以 0014 为准。
