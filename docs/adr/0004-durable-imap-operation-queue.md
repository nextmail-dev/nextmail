# ADR 0004：持久化 IMAP 操作队列与安全删除降级

状态：已采纳

日期：2026-07-13

## 背景

第四阶段需要让已读、星标、移动、复制、删除、归档、Sent 归档和 Drafts 上传在离线、断线、进程退出及服务器能力差异下保持一致。直接从 React 调用 IMAP 会使本地界面与远端结果耦合，也无法可靠恢复中断操作。传统 `EXPUNGE` 会清理当前文件夹内所有带 `\\Deleted` 的邮件，可能删除其他客户端标记的无关邮件。

## 决策

- 用户写操作在同一个 SQLite 事务中更新本地投影并写入 `pending_operations`，React 立即读取乐观结果。
- Rust `AccountSupervisor` 是唯一的远端执行者；进程退出时的 `running` 操作在下次启动恢复为 `queued`。
- 持久身份只使用文件夹、UIDVALIDITY 和 UID，消息序号不进入数据库或公共 DTO。
- Flags 保存“增加/移除某个 Flag”的意图，不覆盖完整 Flags。CONDSTORE 可用时使用 `UNCHANGEDSINCE`，冲突后读取最新 MODSEQ 并只重放一次增量。
- MOVE 优先使用 `UID MOVE`。缺失 MOVE 时使用 `UID COPY` 后标记源消息 `\\Deleted`；只有 UIDPLUS 可用时才执行 `UID EXPUNGE`。
- 缺失 UIDPLUS 时不执行宽泛 EXPUNGE，操作记为 `cleanup_pending`，由服务器或其他客户端稍后安全清理。
- SMTP 成功与 Sent APPEND 是两个状态；Sent 归档按 Message-ID 检查幂等性，APPEND 失败不能触发再次 SMTP 发送。
- NextMail 草稿使用稳定的 `X-NextMail-Draft-ID`；替换时先 APPEND 新版本，再按 UID 清理旧版本。

## 结果

本地交互不等待网络，操作可以重启恢复，并能在能力较弱的服务器上保持“不误删”优先。代价是 SQLite 增加队列状态机，服务器不支持 UIDPLUS 时可能暂时保留带 `\\Deleted` 的邮件；界面必须明确展示该状态。

