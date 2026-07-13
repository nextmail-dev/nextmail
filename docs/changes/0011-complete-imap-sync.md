# 变更 0011：完整 IMAP 同步语义

日期：2026-07-13

## 后台同步

- 启动同步升级为常驻单账户 `AccountSupervisor`：启动即对账，Inbox 支持 IDLE，缺失 IDLE 时 60 秒轮询，其他文件夹及队列最迟每 5 分钟检查。
- IDLE 监听连接与命令连接隔离；手动“收取”、本地待办和网络重试只唤醒 Supervisor，不从前端建立协议连接。
- 同步除新 UID 外会对账全部 UID/Flags、远端删除、UIDVALIDITY 与 MODSEQ；服务器支持 CONDSTORE 时启用 `SELECT (CONDSTORE)`。
- 重连从 2 秒指数退避到最多 5 分钟，任一成功同步后重置。

## 离线写操作

- 新增 `pending_operations`、本地隐藏投影和账户文件夹角色覆盖迁移。
- 已读/未读、星标、移动、复制、删除和归档在同一数据库事务中先更新本地投影，再由 Worker 顺序写回 IMAP。
- Flags 使用增删意图；CONDSTORE 冲突会读取最新 MODSEQ 后重放一次，不覆盖其他客户端修改的无关 Flags。
- MOVE/UIDPLUS 按服务器 Capability 选择；缺失 UIDPLUS 时绝不执行全文件夹 EXPUNGE，并显示待服务器清理提示。
- `running` 操作在异常退出后恢复；临时错误退避重试，最终错误回滚本地投影并允许用户显式重试。

## Sent、Drafts 与文件夹映射

- 账户管理增加 Sent、Drafts、Trash、Archive 映射；SPECIAL-USE/可靠内置角色仍作为默认值，用户覆盖优先。
- SMTP 成功在同一事务中创建独立 Sent APPEND 操作。Sent APPEND 按 Message-ID 幂等检查，失败只重试归档，不会再次发信。
- 本地草稿停止编辑 10 秒或关闭窗口时上传最新版本；队列会合并尚未执行的旧版本。
- 远端草稿带稳定 `X-NextMail-Draft-ID`，替换时先 APPEND 新版本并确认，再安全清理旧 UID。
- 其他客户端创建的 Drafts 可从阅读页转换为本地 Tiptap 草稿；正文与附件先落本地，再打开写信窗口。

## 前端

- 工具栏“收取”启用并显示同步状态。
- 打开未读邮件自动加入已读队列；邮件列表可切换星标，并显示待同步图标。
- 阅读页支持已读/未读、星标、移动、复制、删除和存在映射时的归档。
- 后台最终失败提供重试；安全清理待办显示明确说明。

## 验证

- Rust Repository 测试覆盖乐观 Flags、持久认领、失败回滚和迁移。
- 协议测试覆盖条件 STORE 语法、MIME/编码和 HTML 安全回归。
- 发件测试覆盖稳定远端草稿身份头。
- `cargo test --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`pnpm test` 和 `pnpm build` 通过。
- 按新的本地调试约定，本轮不执行 Tauri 完整构建；桌面联调由 `pnpm tauri dev` 完成。

