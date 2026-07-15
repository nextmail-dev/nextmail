# ADR 0006：Rust 收敛为单一 Tauri package

状态：已接受

日期：2026-07-15

## 背景

第二阶段曾用 `nextmail-core`、`nextmail-storage`、`nextmail-protocols` 和 `src-tauri` 四个 Cargo package 强化依赖边界。随着实现推进，这些包没有独立发布、独立版本、单独部署或供其他二进制复用的需求，却让仓库同时出现根 Cargo Workspace 和 `src-tauri` 两个 Rust 工作入口。

本机因此保留了约 18.81 GB 的根 `target` 与约 7.63 GB 的 `src-tauri/target`。开发命令、lockfile、迁移相对路径和增量缓存归属也更容易混淆。

## 决策

- Rust 只保留 `src-tauri` 一个 Cargo package。
- 原 `nextmail-core`、`nextmail-storage` 和 `nextmail-protocols` 源码分别迁入 `src-tauri/src/core`、`storage` 和 `protocols` 内部模块。
- SQLx 嵌入式迁移迁入 `src-tauri/migrations`。
- 唯一的 `Cargo.toml`、`Cargo.lock` 和 `target` 位于 `src-tauri`；仓库根目录不再是 Cargo Workspace。
- 不因取消子 crate 而取消职责边界。`core` 继续禁止依赖 Tauri、SQLx 与具体协议库，协议和存储实现继续通过自有类型及 ports 接入。

除非未来出现独立发布、独立版本、独立部署或被多个宿主复用的明确需求，不重新引入根 Cargo Workspace。

## 影响

- Cargo 只有一套依赖解析、lockfile 和增量构建缓存，日常命令统一从 `src-tauri` 执行。
- SQLx 迁移和 Rust 构建上下文与 Tauri manifest 保持一致。
- 不再由 crate 可见性提供最强的编译期隔离；改由模块可见性、依赖方向审查、稳定 DTO 和模块级测试维持边界。
- 历史 ADR 0001 被本决策取代，但保留以解释演进过程。
