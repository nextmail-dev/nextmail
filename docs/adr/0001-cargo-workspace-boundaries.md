# ADR 0001：第二阶段引入 Cargo Workspace

状态：已接受

日期：2026-07-12

## 背景

第一阶段只有一个桌面宿主和首次启动用例，包内模块已经足够。第二阶段开始引入长期运行 Worker、IMAP、MIME、SQLite Repository 和安全内容存储，协议与基础设施依赖需要独立编译和测试边界。

## 决策

拆分 `nextmail-core`、`nextmail-storage`、`nextmail-protocols` 和 `src-tauri`。核心只定义领域、用例和 ports；具体协议、数据库和 Tauri 类型不得进入核心。

不按每个小功能创建 crate，也不创建 Go/Python Sidecar。

## 影响

- 邮件核心可在无 WebView、无真实服务器的环境中测试。
- SQLx 和协议依赖不会泄漏到领域层。
- 初次机械迁移会增加工作量，必须在增加同步功能前保持第一阶段测试全部通过。
