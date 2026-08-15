# 网易邮箱 IMAP ID 身份标识

状态：已通过手动验收（v0.7.0 发布）

## 目标

修复网易系邮箱（163.com / 126.com / 188.com 等）IMAP 同步失败的问题：网易服务器对未发送 RFC 2971 `ID` 命令的客户端拒绝邮箱操作，返回 `EXAMINE Unsafe Login. Please contact kefu@188.com for help`，表现为 `sync.mailbox_open_failed` 循环失败、邮件无法入库。登录后在服务器声明支持 `ID` 能力时发送客户端身份标识。

## 背景与诊断

实机日志中 188 账户（account_id 8f3ca7ef）同步始终以 `sync.mailbox_open_failed` 失败，服务器响应文本为 `Unsafe Login. Please contact kefu@188.com for help`。网易官方说明：IMAP 接入必须携带 IMAP ID（由 name、version、vendor 等 key-value 组成），否则返回该报错；常用客户端（Outlook、Foxmail 等）不受影响。此前连接建立后从未发送 `ID`。

## 范围

- `protocols` 新增共享助手 `send_imap_id_if_supported`：登录后刷新 CAPABILITY，声明 `ID` 时发送 `ID ("name" "NextMail" "version" "<Cargo 包版本>" "vendor" "NextMail")`；失败（能力刷新失败、ID 被拒）仅记警告不中断连接——无门禁的服务器行为不变，有门禁的服务器在邮箱打开时按既有错误路径暴露问题。
- 同步会话路径（`protocols/imap/connection.rs::login`）与连接测试路径（`adapters/mail_connection.rs::authenticate_imap`）都调用该助手，账户添加/编辑时的连接测试同样通过。
- 新增脚本化会话测试：服务器声明 `ID` 时客户端发送含正确 name/version 的 ID 命令并正常消费响应；未声明时完全不发送。
- 新增 iteration 文档与 README 索引。

## 非目标

- 不发送 `support-email`（无官方支持邮箱，网易文档中为示例字段）；不修改登录顺序、会话预算或重试策略。
- 不把 ID 失败升级为硬错误；不做仅按域名（163 等）触发。
- 不影响其他邮箱提供商的连接行为（无 `ID` 能力则零变化）。

## 验证门禁

- `cargo fmt --all -- --check`、`cargo clippy --offline --locked --all-targets -- -D warnings`、`cargo test --offline --locked`。
- 脚本化测试覆盖“有 ID 能力发送 / 无 ID 能力不发送”两条路径。
- Windows 实机：163/188 账户同步正常完成、邮件入库；连接测试通过；其他邮箱（QQ 等）行为不变。

## 实施结果

- `protocols::send_imap_id_if_supported` 已实现并在两条连接路径调用；ID 值为 `name=NextMail`、`version=env!("CARGO_PKG_VERSION")`、`vendor=NextMail`。
- 两个脚本化会话测试已添加并通过。

## 自动验证

- `cargo fmt --all -- --check`、`cargo clippy --offline --locked --all-targets -- -D warnings` 通过。
- `cargo test --offline --locked`：186 项通过（含新增 2 项脚本化会话测试）。
- `git diff --check` 通过。

## 手动验收重点

1. 用 163 或 188 邮箱手动收取：同步正常完成，不再出现 `Unsafe Login` 相关的 `sync.mailbox_open_failed`。
2. 账户管理中添加/测试 163/188 账户：连接测试通过。
3. QQ 等其他邮箱同步与连接测试行为与之前一致。
