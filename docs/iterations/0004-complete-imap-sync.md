# 第四阶段：完整 IMAP 同步语义

状态：已通过自动验证与用户真实账户手动验收。

## 实施结果（2026-07-13）

已完成常驻 Supervisor、Inbox IDLE/轮询回退、5 分钟全文件夹对账、UID/Flags/MODSEQ/远端删除同步、手动收取、SQLite 乐观操作队列、已读/星标/移动/复制/删除/归档、安全 MOVE/UIDPLUS 降级、失败回滚与重试、系统文件夹映射、Sent 幂等归档、本地/远端 Drafts 版本替换及服务器草稿导入。

QRESYNC 仍不作为本阶段基线；实现使用 CONDSTORE 加完整 UID/Flags 对账。批量命令边界已经支持多个消息 ID，当前界面先提供单封邮件操作，批量选择交互未提前扩展。

## 一、阶段目标

在现有单账户、本地优先阅读与持久化 SMTP 发件基础上，完成持续接收、远端状态对账、邮件写操作、离线操作队列、系统文件夹映射和冲突恢复。

- 应用启动先展示 SQLite 本地数据，再由 Rust 账户 Supervisor 持续同步；React 不持有协议连接。
- 收件箱优先使用 IMAP IDLE，其他文件夹定时轮询；服务器不支持 IDLE 时自动回退轮询。
- 已读/未读、星标、移动、复制、删除和归档先在本地事务中更新，再由后台队列写回服务器。
- SMTP 成功后独立将已持久化的原始 MIME 追加到 Sent；归档失败只重试归档，绝不再次 SMTP 发信。
- 保存 UIDVALIDITY、UID 和 MODSEQ 等远端身份，按服务器能力选择安全命令，不用易漂移的消息序号作为持久标识。

本阶段继续保持单账户。多账户并发 Supervisor、OAuth、POP3、模板/签名库、全文搜索和会话聚合仍按总体路线留在后续阶段。

## 二、同步运行模型

### 账户 Supervisor

Rust 端新增单账户 `AccountSupervisor`，拥有两个互相独立的 IMAP 会话：

- `watch session`：只负责收件箱 IDLE。IDLE 期间不复用该连接执行查询或写操作。
- `command session`：负责文件夹发现、增量拉取、Flags 对账、APPEND、COPY、MOVE、STORE 和安全 EXPUNGE。

Supervisor 内部串行化同一账户、同一文件夹的状态变更，避免两个 Worker 同时推进 UID/MODSEQ 水位。应用启动、手动“收取”、IDLE 通知、定时轮询、联网恢复和本地待办写操作都只向 Supervisor 投递原因可合并的任务，不直接创建额外同步线程。

### 调度和退避

- 启动后立即做一次轻量文件夹状态检查；本地查询不等待网络结果。
- 支持 IDLE 时仅持续监听 Inbox；连接最迟每 25 分钟主动续订一次。
- 其他可选文件夹每 5 分钟检查一次；用户打开文件夹时触发一次高优先级检查。
- 不支持 IDLE 时 Inbox 每 60 秒轮询；应用离线或系统网络不可用时暂停主动连接。
- 重连退避为 2、5、15、30、60 秒，随后以 5 分钟为上限并加入抖动；任一成功命令重置退避。
- 工具栏“收取”变为可用，唤醒当前等待并合并重复请求，不并行启动多次全量同步。

## 三、服务器能力与安全降级

登录后缓存每次连接的 Capability，并在重连后重新获取。所有修改使用 UID 命令。

| 能力 | 首选行为 | 缺失时行为 |
| --- | --- | --- |
| `IDLE` | Inbox 持续监听变化 | 60 秒轮询 |
| `CONDSTORE` | `SELECT (CONDSTORE)`，保存 MODSEQ，按变化对账 Flags | 周期性拉取当前 UID/FLAGS 集合并比较 |
| `MOVE` | `UID MOVE` | `UID COPY` 后对源消息加 `\\Deleted` |
| `UIDPLUS` | 使用 COPYUID 关联目标 UID；按 UID 精确 EXPUNGE | 不执行可能影响其他邮件的全文件夹 EXPUNGE |

关键安全规则：

- MOVE 不可用但 COPY 成功时，先确认目标副本再标记源消息 `\\Deleted`。若 UIDPLUS 也不可用，只记录 `remote_cleanup_pending`，不自动执行宽泛 EXPUNGE。
- 普通“删除”优先移动到映射的 Trash；已在 Trash 中永久删除时，仅在 UIDPLUS 可用时按 UID 精确清理。能力不足时只标记 `\\Deleted` 并提示服务器稍后完成清理。
- “归档”只有在账户明确映射 Archive 文件夹时才显示；不根据文件夹名称猜测 Gmail 标签或通过移除 `\\Inbox` 模拟归档。
- QRESYNC 不作为本阶段验收前提。数据模型继续保存 MODSEQ，首轮以 async-imap 已有的 CONDSTORE 和 UID 对账完成冲突恢复；后续只有在 Adapter 级兼容测试通过后才启用 QRESYNC 扩展。

## 四、增量拉取与远端对账

每次选择文件夹后读取 UIDVALIDITY、UIDNEXT 和可用的 HIGHESTMODSEQ：

1. UIDVALIDITY 未改变时，拉取高于本地 `last_uid` 的新 UID，并按现有 90 天正文策略落库。
2. 支持 CONDSTORE 时按 MODSEQ 获取变化的 Flags；否则获取文件夹当前 UID/FLAGS 集合做差异比较。
3. 周期性完整 UID 集合对账用于发现其他客户端删除的消息位置；同一规范化消息位于其他文件夹时不删除消息本体。
4. UIDVALIDITY 改变时废弃该文件夹旧位置身份并重新枚举；保留原始 EML 与规范化消息，通过 Message-ID、大小、日期和内容哈希做保守复用，不强行匹配不确定邮件。
5. 所有一批数据库变化在事务中提交，增加文件夹/消息修订号后再发事件。事件只携带账户、文件夹、消息 ID 与修订号，前端据此失效查询。

## 五、离线操作队列

新增 `pending_operations`：

- 身份：操作 ID、账户槽、操作类型、消息 ID、源/目标文件夹 ID。
- 远端基线：UID、UIDVALIDITY、可选 base MODSEQ。
- 意图：Flags 增删集合或移动/复制/删除目标，不保存一份覆盖服务器全部状态的快照。
- 生命周期：`queued | running | retry_wait | needs_reconcile | succeeded | failed`。
- 恢复信息：尝试次数、下次执行时间、稳定错误码、创建和更新时间。

用户操作在一个 SQLite 事务中完成“更新本地投影 + 写入 pending operation”，界面立即显示结果。Worker 按账户和文件夹顺序重放；进程退出时遗留的 `running` 在下次启动恢复为 `queued`。

冲突规则：

- Flags 使用增量意图，例如“增加 `\\Seen`”而不是覆盖整组 Flags。CONDSTORE 条件失败时重新读取最新 Flags，再应用同一增量一次；连续冲突转为 `needs_reconcile`。
- 移动/删除时如果源 UID 已不存在，先检查目标位置、COPYUID、Message-ID 和内容哈希；确认目标存在则视为成功，否则重新同步并给出稳定失败状态。
- UIDVALIDITY 与队列基线不一致时禁止直接重放旧 UID，必须先重新定位消息；无法唯一定位则失败，不猜测目标。
- 永久错误使本地投影回到最新远端状态并在界面显示失败；网络/服务器临时错误保留乐观状态并退避重试。

## 六、文件夹角色、Sent 与 Drafts

新增账户级文件夹角色覆盖表，支持 `sent | drafts | trash | archive`，优先读取 IMAP SPECIAL-USE，无法可靠识别时由账户设置显式选择。Inbox 仍由协议固定识别。

### Sent 归档

- SMTP `send_job` 成功后创建独立 `append_sent` 操作，引用第三阶段已经持久化的不可变 MIME。
- 向映射的 Sent 执行 APPEND，并设置 `\\Seen`；成功后记录远端 UID（若服务器返回）。
- APPEND 失败不回滚 SMTP 成功状态、不重新发信，只显示“邮件已发送，正在保存到已发送”并独立重试。
- 未映射 Sent 时保留本地已发送记录并提示用户配置，不猜测目标文件夹。

### Drafts 同步

- 本地 Tiptap 三格式草稿仍是 NextMail 编辑时的规范来源。
- NextMail 草稿在停止编辑 10 秒或关闭写信窗口时排队上传到映射 Drafts；使用 `X-NextMail-Draft-ID` 关联版本。
- 更新远端草稿采用“先 APPEND 新版本并确认成功，再安全删除旧版本”，不在每次 800 毫秒本地自动保存时访问网络。
- 其他客户端创建的 Drafts 作为普通远端草稿同步；首次编辑时从 HTML/纯文本转换成新的编辑器文档，原始 EML 保留用于恢复。
- 未映射 Drafts 时继续可靠保存本地草稿，不阻塞写信。

## 七、公共命令、事件与界面

新增或扩展命令：

- `sync_now(account_id, mailbox_id?)`
- `set_message_read(account_id, message_ids, read)`
- `set_message_flagged(account_id, message_ids, flagged)`
- `move_messages(account_id, message_ids, destination_mailbox_id)`
- `copy_messages(account_id, message_ids, destination_mailbox_id)`
- `delete_messages(account_id, message_ids)`
- `archive_messages(account_id, message_ids)`
- `retry_pending_operation(account_id, operation_id)`
- `list_pending_operation_status(account_id)`
- `set_mailbox_role_mapping(account_id, role, mailbox_id?)`

命令只接收稳定 ID 和业务意图。React 不接触 UID、MODSEQ、SQL 或协议错误。事件包括 `mailbox-revision-changed`、`message-revision-changed`、`sync-progress-changed` 和 `pending-operation-changed`，不携带正文或完整消息列表。

本阶段界面变化：

- 工具栏“收取”显示同步/完成/离线状态。
- 邮件列表支持已读/未读与星标；阅读器和批量选择提供移动、复制、删除及可用时的归档。
- 设置中的账户管理增加 Sent、Drafts、Trash、Archive 文件夹映射。
- 乐观操作显示轻量待同步状态；最终失败提供重试，不把服务器原始响应暴露给用户。
- 后续体验修正将待办唤醒与完整同步分开：小操作只重放队列；后台 IDLE/轮询静默更新，只有启动和手动收取显示进度。
- 主工作区支持文件夹栏/邮件列表拖动和文件夹图标折叠；顶部工具栏承载基础回复、回复全部、转发和复制。
- 基础回复/转发在本阶段补齐，线程头随草稿与发送 MIME 持久化；后续“模板与签名”阶段仅继续扩展对应模板和签名规则。

## 八、模块实施顺序

1. 增加队列、角色映射、同步水位和远端草稿位置迁移，完成 Repository 状态机测试。
2. 扩展自有 `ImapProvider`，封装 Capability、IDLE、CONDSTORE、UID STORE/COPY/MOVE/EXPUNGE 与 APPEND，不向应用层暴露 async-imap 类型。
3. 实现 AccountSupervisor、任务合并、轮询/IDLE 切换、退避和启动恢复。
4. 实现增量 Flags、远端删除、UIDVALIDITY 变化和完整 UID 对账。
5. 实现本地乐观事务、pending operation Worker、冲突恢复及安全降级。
6. 接入工具栏、列表/阅读器操作、待同步状态与文件夹映射设置。
7. 接入 SMTP 后 Sent APPEND 与远端 Drafts 版本同步。
8. 更新架构、ADR 与变更记录，完成自动验证后交付手动验收。

## 九、自动测试与兼容门禁

- Repository：本地投影与队列同事务、崩溃恢复、幂等重放、角色映射和水位迁移。
- Adapter：进程内 IMAP 测试服务或固定协议会话覆盖 IDLE、无 IDLE、CONDSTORE 冲突、MOVE、无 MOVE、UIDPLUS 与无 UIDPLUS。
- Supervisor：任务合并、退避重置、IDLE 续订、轮询回退、网络恢复和关闭清理。
- 同步：新 UID、Flags、远端删除、UIDVALIDITY 改变、重复事件和重新连接均不产生重复消息。
- 写操作：已读、星标、复制、移动、删除、归档的成功、临时失败、永久失败和冲突路径。
- 发件归档：SMTP 只执行一次；Sent APPEND 失败与重启重试不会再次发送。
- 草稿：版本替换先增后删，断线时本地草稿完整，重新上线后最终只保留可识别的最新版本。
- 继续执行 Rust 单元/集成测试、Clippy、前端测试和 TypeScript/Vite 构建；真实账户及桌面行为由用户通过 `pnpm tauri dev` 手动验收，日常不重复执行 Tauri 完整构建。

## 十、手动验收标准

- 启动后立即读取本地邮件；服务器在线时持续收到新邮件，IDLE 不可用的账户能自动轮询。
- 点击“收取”不会生成并发重复同步；断网和恢复不需要重启应用。
- 在其他客户端修改已读、星标、移动或删除后，NextMail 能在合理时间内对账且不重复邮件。
- 离线执行已读、星标、移动和删除时界面立即更新；联网后写回服务器，重启应用不会丢失待办操作。
- 不支持 MOVE/UIDPLUS 的服务器上不会因宽泛 EXPUNGE 删除无关邮件，并能清楚显示待服务器清理状态。
- UIDVALIDITY 改变、MODSEQ 冲突和消息已被其他客户端移动时能够恢复或给出稳定失败，不错误操作另一封邮件。
- SMTP 成功但 Sent APPEND 失败时只重试归档，收件人只收到一封邮件。
- 本地草稿始终可恢复；配置 Drafts 后可以跨客户端看到已上传草稿，更新中断不丢失最后成功版本。
- 密码、原始服务器响应、内部路径、正文和完整邮件数据不进入事件或普通日志。
- Windows 10 22H2+ 实机通过；macOS 只在实际 Runner/设备验证后声明通过。
