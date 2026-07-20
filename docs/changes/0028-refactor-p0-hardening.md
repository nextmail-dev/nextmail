# 0028 P0 正确性与同步热路径重构

日期：2026-07-20

## 变更

- 修复后台 `message-content-changed` 事件无法命中邮件详情查询的问题，并集中邮件详情 query key。
- 为 `upsert_message` 增加完整 SQLite 事务、批量附件 UPSERT 和失败回滚测试。
- 将 `reconcile_mailbox` 的逐位置待办查询与删除改为临时 UID 集合加单条集合 DELETE。
- 新增进程级原生根证书/rustls 配置缓存，IMAP 同步与账户连接测试共用。
- 将新邮件正文和缺失正文回填改为每批最多 100 个 UID 的 IMAP FETCH，移除逐封网络往返。

## 保持不变

- React 仍只通过稳定业务命令访问本地视图；公共 DTO 与错误码未改变。
- 原始 EML、附件内容、账户隔离、HTML 清洗和凭据边界未放宽。
- Rust 仍为单一 `src-tauri` Cargo package；未引入新依赖、迁移或产品功能。

## 验证

- 定向前端：`MessageViewer.test.tsx` 1 项通过，TypeScript 检查通过。
- 定向存储：5 项通过。
- 定向协议与 TLS：14 项通过。
- `pnpm test`：14 个测试文件、29 项测试全部通过。
- `pnpm build`：TypeScript 与 Vite 生产构建通过；现有主入口 chunk 仍有大于 500 kB 的提示，留待 P2/P3 前端拆分批次处理。
- `cargo fmt --all -- --check` 通过。
- `cargo test --offline --locked`：56 项测试全部通过。
- `cargo clippy --offline --locked --all-targets -- -D warnings` 通过。
- 未执行 Tauri bundle 构建；`dist` 与 `src-tauri/target` 按项目约定保留为增量构建缓存。

## 手动验收

- 2026-07-20：用户实机验收通过。
