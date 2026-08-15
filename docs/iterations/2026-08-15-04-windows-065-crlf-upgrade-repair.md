# Windows 0.6.5 存量数据库升级修复

状态：已通过手动验收（v0.6.7 发布）

## 目标

修复 Windows 0.6.5 及更早版本建库的用户在升级到 0.6.6+ 时启动报 `data_directory.database_migration_failed` 的问题。这些 Windows 构建（当时没有 `.gitattributes`，Windows 检出为 CRLF）内嵌 CRLF 基准的迁移校验和，其数据库无法通过 0.6.6 起的 LF 基准校验。0.6.6 已把构建统一为 LF，本计划补上存量数据库的过渡修复。

## 背景与诊断

上一轮（2026-08-15-02）固定迁移行尾并修正了本机数据库，但只覆盖单机链路：官方 Windows CI 的 0.6.5 构建同样内嵌 CRLF 校验和（Windows runner 默认 `core.autocrlf=true`）。用户报告另一台电脑从 0.6.5 升级 0.6.6 后仍迁移失败。

复核结论：

- main 上 29 个迁移 blob 全部为 LF，0.6.6 官方构建内嵌 LF——提交内容与构建无误。
- 失败来自 0.6.5 Windows 构建写入的 CRLF 校验和行；Linux/macOS 0.6.5 构建（LF）不受影响。
- sqlx 0.9 的 `_sqlx_migrations.checksum` 存 SHA-384 原始 48 字节 digest（BLOB 列），校验按字节比较。

另外发现上一轮声称的数据库备份 `content.sqlite.backup-2026-08-15-lf-fix` 实际为 0 字节（backup API 未写入），已确认本机主库 29 行现为 LF 且应用正常，无数据风险；该空文件可删除。

## 范围

- 新增 `storage/migration_repair`：打开数据库后、`MIGRATOR.run` 之前，把 `_sqlx_migrations` 中 29 个已知 CRLF 校验和行改写为 LF digest；`WHERE` 精确匹配已知 CRLF 值，幂等，只影响完全匹配的行；表不存在或个别行出错时跳过，不阻断启动，修复数量写入警告日志。
- `MIGRATOR` 改为 `pub(crate)` 供测试引用；`open_pool` 在连接建立后调用修复（best-effort）。

## 非目标

- 不修改迁移文件、不新增迁移、不改产品行为。
- 不处理未来新增迁移的 CRLF 变体（`.gitattributes` 已从源头杜绝）。
- 不引入新依赖；hex 解码为模块内小函数。

## 验证门禁

- `cargo fmt --all -- --check`、`cargo clippy --offline --locked --all-targets -- -D warnings`、`cargo test --offline --locked`。
- 单元测试：校验和表与编译期内嵌 `MIGRATOR` 的 LF 值逐一一致（防文件改名/改内容后表漂移）；CRLF 行改写后 `MIGRATOR.run` 验证通过；LF 行保持不变；新库无表时静默跳过。
- Windows 实机：在出问题的电脑上安装修复版本，正常打开 0.6.5 建的数据库。

## 实施结果

- `migration_repair.rs` 内置 29 组（version, CRLF hex, LF hex），运行时解码为 48 字节 digest 执行精确 `UPDATE`；`open_pool` 连接后调用。
- 本机数据库 29 行已为 LF（修复 no-op），无需再次手动处理。

## 自动验证

- `cargo fmt --all -- --check`、`cargo clippy --offline --locked --all-targets -- -D warnings` 通过。
- `cargo test --offline --locked`：188 项通过（新增 4 项）。
- 本机主库 29 行与 LF digest 逐一比对一致。

## 手动验收重点

1. 在出问题的电脑上安装含本修复的版本：启动正常进入主界面，日志不再出现 `data_directory.database_migration_failed`；若出现 `rewrote legacy CRLF migration checksums to LF values` 警告说明修复已执行且后续启动不再出现。
2. 本机启动、同步与阅读行为与之前一致。
