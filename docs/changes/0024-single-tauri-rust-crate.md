# 变更 0024：Rust 收敛为单一 Tauri package

日期：2026-07-15

## 变更内容

- 移除仓库根 Cargo Workspace、根 `Cargo.lock` 和三个 `crates/nextmail-*` package。
- 将领域核心、协议、存储源码迁入 `src-tauri/src/core`、`protocols` 和 `storage`，保留原有模块职责与稳定类型边界。
- 将 SQLx 嵌入式迁移统一迁入 `src-tauri/migrations`。
- 合并 Rust 依赖到 `src-tauri/Cargo.toml`，并把唯一 lockfile 固定在 `src-tauri/Cargo.lock`。
- 删除约 18.81 GB 的历史根 `target`；保留 `src-tauri/target` 作为唯一增量构建缓存。
- 更新架构、总体计划与 ADR，明确后续阶段不得无实际复用需求重新建立根 Cargo Workspace。

## 行为与数据影响

本次只调整 Rust 工程组织方式，不改变产品功能、命令 DTO、SQLite schema、数据目录格式、账户配置或协议行为。既有迁移文件内容保持不变。

## 验证

- Cargo metadata 只包含 `nextmail` 一个 package，workspace root 和 target directory 均位于 `src-tauri`。
- 从 `src-tauri` 执行 Rust 格式检查、全目标 Clippy 和单元测试。
- 不执行完整 Tauri 或前端构建，由用户继续使用 `pnpm tauri dev` 做桌面联调。
