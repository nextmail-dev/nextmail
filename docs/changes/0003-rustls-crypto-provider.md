# 0003 - 固定 rustls CryptoProvider

日期：2026-07-12

## 问题

账户验证创建 TLS 配置时发生进程 panic。直接依赖的 `rustls`/`tokio-rustls` 默认启用了 `aws-lc-rs`，同时 `reqwest` 和 `lettre` 启用了 `ring`，rustls 无法从两个已编译 provider 中自动选择进程级默认实现。

## 修复

- `rustls` 和 `tokio-rustls` 关闭默认特性，显式统一启用 `ring`、日志和 TLS 1.2 支持。
- 在 Tauri Builder 和 Tokio Worker 启动前显式安装进程级 `ring` CryptoProvider。
- 增加单元测试，确保进程级 CryptoProvider 能在启动阶段完成安装。

该变更不放宽证书链或主机名验证，也不增加忽略证书错误的入口。
