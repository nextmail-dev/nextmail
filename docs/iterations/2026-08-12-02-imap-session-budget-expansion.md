# 扩充账户级 IMAP 会话预算

状态：等待手动验收

## 目标

将每账户并发 IMAP 会话上限由 3 提升至 6，完整同步 worker 由 2 提升至 4，交互容量由 1 提升至 2，提升单账户同步并发度与同步期间的交互并行度。

## 范围

- `ACCOUNT_SESSION_LIMIT` 3 -> 6，`SYNC_SESSION_COUNT` 2 -> 4。
- 全局网络许可 `network_limit` 保持 2 不变。
- 更新 `provider.rs` 同步租约注释、`project.md` §5 与 §10、ADR 0014 修订说明。
- 更新 `session_budget` 单元测试以反映新预算与"同步 4 + 交互 2 = 6，第 7 个等待"。

## 决策与风险

- 每账户连接数峰值从 3 升至 6，对并发连接数较严的 IMAP 服务器风险上升（可能报 "too many connections" 或重置已有连接）。ADR 0014 原本为规避此问题而保守取 3/2；本次提升以实机验收为前提，若严格服务器出现问题需回调或做成可配置。
- 仍保持有界预算与按需扩缩、用完即关，未引入长期连接池；`core`/application/Command/Repository/前端契约不变。
- 同步 worker 翻倍后，单文件夹头/正文回填并发拉取路数由 2 升至 4；SQLite upsert 仍由本地 `write_lock` 串行，网络并行度提升但写库不变。

## 验证门禁

- `session_budget` 单元测试通过：同步 4 + 交互 2 可取得，第 7 个等待。
- `cargo fmt --all -- --check`、`cargo test --offline --locked`、`cargo clippy --offline --locked --all-targets -- -D warnings`、`git diff --check` 通过。
- 实机验收：完整同步期间连续打开缺正文邮件、触发待办重放，确认同步与交互均完成，日志无连接重置或 "too many connections"。

## 验证结果

- 2026-08-12：自动验证通过，等待用户手动验收。
- `cargo fmt --all -- --check` 通过。
- `cargo test --lib` 通过，共 170 项测试（含更新的 `keeps_interactive_slots_while_sync_workers_are_active`）。
- `cargo clippy --locked --all-targets -- -D warnings` 通过，无警告。
- `git diff --check` 通过。
- 说明：`--offline` 因本机 registry 索引缺 `tauri-plugin-updater` 无法解析，本轮以非离线模式运行 `--locked`。
- 实机验收待做：完整同步期间连续打开缺正文邮件、触发待办重放，确认无连接重置或 "too many connections"；若严格服务器出问题需回调或改为可配置。
