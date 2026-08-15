# QQ 退信邮件 BODYSTRUCTURE 解析失败导致同步中断修复

状态：已通过手动验收（v0.6.7 发布）

## 目标

修复收取 QQ 邮箱退信报告（multipart/report）邮件时整账户同步失败、邮件无法保存入库的问题。QQ 服务器在这类邮件的 `BODYSTRUCTURE` 中把部分 part 的 body-fld-enc（transfer encoding）字段返回为 `NIL`，超出 RFC 3501 文法（该字段应为 string），async-imap 依赖的 imap-proto 0.16.7 按严格文法解析失败，整条 FETCH 响应无法解析。

## 背景与诊断

实机日志：QQ 账户（account_id 109ed6c6）同步在 UID 6616（14:01 到达的 `X-QQ-MAIL-TYPE: bulletin` 退信报告）处报 `sync.message_fetch_failed`，同步中止；该 UID 永远留在差集中，每次同步都在同一封邮件重演，其余邮件无法入库。日志中错误表面为 `TakeWhile1 during parsing of "* 1 FETCH (...)"`（input 指向响应开头），但这是 nom `alt` 只返回最后一个分支错误的假象；真实失败点是响应末尾的 `BODYSTRUCTURE`。

用 imap-proto 0.16.7 直接回放日志中的原始字节复现，并二分定位：

- `BODYSTRUCTURE` 中 body-fld-enc 为 `"7BIT"`/`"BASE64"` 等 string 时解析正常；
- 为 `NIL` 时 `body_encoding`（只接受 quoted string）失败，整条响应解析失败；
- 失败还使 async-imap 连接流报废，该 worker 会话上后续所有批次一并失效。

## 范围

- `fetch_summaries_worker` 每批拆成两条 FETCH：头部 `(UID FLAGS [MODSEQ] INTERNALDATE RFC822.SIZE BODY.PEEK[HEADER])` 先逐封提交（保持 0.6.5 的断线不丢已收邮件语义），随后单独取 `(UID BODYSTRUCTURE)`。
- `BODYSTRUCTURE` 取回后把附件元数据合并写回已落库邮件；该命令失败时降级：记录警告（不含服务器原始响应），本 worker 提前结束，会话标记为不可用，同步整体正常完成。已提交头部的邮件不再出现在下轮差集中，缺失的只是附件元数据；剩余批次由下轮差集续传，一次额外同步后收敛。
- 正文预取同样按会话降级：结构获取失败且完整消息回退失败（毒响应同时杀死会话）时，当前邮件保持待取，剩余队列在同轮重派发给存活会话。
- `sync_folder` 后续阶段（flags reconcile、正文预取）跳过不可用会话，不因单个会话报废再次中断整轮同步。
- 单封按需结构获取的失败统一映射为选择性获取不支持，走既有完整消息回退路径。
- 新增脚本化会话测试：正常响应附件元数据合并且会话可用；QQ 风格 NIL 编码响应时头部已落库、附件元数据缺失、worker 返回会话不可用；正文预取毒响应后剩余队列重派发。

## 非目标

- 不修改迁移、数据库 schema、前端或依赖版本；不 fork/patch imap-proto。
- 不引入会话重连机制；不可用会话由下轮同步自然替代。
- 不处理 `map_imap_err` 对 io 错误记录原始响应的既有日志行为（本次新增警告路径不记录原始响应，既有路径另立计划处理）。

## 验证门禁

- `cargo fmt --all -- --check`、`cargo clippy --offline --locked --all-targets -- -D warnings`、`cargo test --offline --locked`。
- 脚本化会话测试覆盖正常、NIL 编码降级与正文预取重派发三条路径。
- Windows 实机：QQ 账户同步不再因该邮件失败，其余邮件正常入库；该封退信邮件可打开（走无 BODYSTRUCTURE 回退路径）。

## 实施结果

- `fetch_summaries_worker` 每批拆为两条 FETCH：头部先逐封解析、落库并发布事件（保持断线不丢已收邮件语义），随后单独取 `(UID BODYSTRUCTURE)`。
- `BODYSTRUCTURE` 取回后仅对带附件的邮件做第二次幂等 upsert 合并附件元数据；该命令失败时记录警告（不含原始服务器响应），本 worker 提前结束并标记会话不可用，同步整体正常完成。
- `sync_folder` 按会话可用性跳过 flags reconcile 与正文预取中的报废会话，不再二次中断整轮同步。
- 正文预取 worker 在结构获取失败且完整消息回退也失败（毒响应同时杀死会话）时，跳过当前邮件并把剩余队列重派发给存活会话，单轮内收敛；毒邮件本身保持待取状态，下轮重试，打开时走交互路径的完整消息回退。
- 单封结构获取（正文/附件按需下载路径）的任何失败统一映射为选择性获取不支持，走既有完整消息回退，不再把解析失败暴露为用户错误。
- `docs/project.md` 同步模型段落补充头部与 `BODYSTRUCTURE` 分命令、失败降级的表述。

## 自动验证

- 新增三个脚本化会话测试（tokio duplex 模拟 IMAP 服务器）：QQ 风格 NIL 编码 `BODYSTRUCTURE` 响应下 3 封头部全部落库、worker 返回会话不可用；正常响应下附件元数据合并进已落库头部且会话可用；正文预取时毒响应杀死会话后剩余队列重派发到存活会话、毒邮件保持待取。
- `cargo fmt --all -- --check`、`cargo clippy --offline --locked --all-targets -- -D warnings` 通过。
- `cargo test --offline --locked`：184 项通过。

## 手动验收重点

1. 用 QQ 账户手动收取：同步正常完成，不再出现 `sync.message_fetch_failed` 循环失败。
2. 打开 UID 6616 的退信报告邮件，正文与附件按回退路径可读。
3. 观察其他邮件（含带附件邮件）列表与附件元数据正常。
