# IMAP 正文与附件选择性下载

状态：已验收

## 目标

依据 IMAP `BODYSTRUCTURE` 区分邮件正文、内联资源与普通附件。同步时先保存邮件头和附件元数据，只下载正文所需 MIME section；普通附件在用户打开、定位或另存为时才下载对应 section，避免正文回填同时传输并保存全部附件。

同时将阅读区附件列表改为紧凑桌面组件：删除“附件：”标题和行内操作按钮，保留单击默认打开，并通过右键菜单提供打开、打开方式、打开文件夹和另存为。

## 范围

- 在既有邮件头 FETCH 中读取 `BODYSTRUCTURE`，解析正文、内联资源和普通附件的 IMAP section。
- 正文回填与打开邮件改为按 section 获取并继续经过现有 Rust MIME 解析、HTML 清洗和内容存储边界。
- 持久化附件 IMAP section；点击未下载附件时只拉取对应 section，成功后复用现有安全打开/保存链路。
- 保留完整原始邮件的按需获取，用于查看源码和无法可靠处理的 MIME 结构回退。
- 附件区压缩尺寸，移除标题和右侧按钮；右键菜单提供打开、打开方式、打开文件夹、另存为，其中“打开方式”本轮仅展示禁用入口。
- 单击附件默认打开；需要下载时在对应附件上展示 loading spinner。
- 删除与固定单击行为冲突的“下载附件后自动打开”偏好；旧配置中的遗留字段由兼容反序列化忽略。
- 更新 `docs/project.md` 的同步、附件与交互约定。

## 非目标

- 不实现附件分块下载、断点续传或下载队列管理。
- 不实现“打开方式”的系统选择器。
- 不改变 HTML 邮件 sandbox、远程图片策略、危险附件确认和账户数据隔离边界。
- 不移除完整原始邮件查看能力。

## 关键约束与回退

- 不使用 `BODY.PEEK[TEXT]` 代替正文选择；multipart 邮件必须按 MIME section 获取。
- 内联 CID 资源属于正文保真依赖，不显示为普通附件；只在正文实际引用且通过既有图片安全校验时使用。
- `BODYSTRUCTURE` 缺失、畸形或 section 获取/解析不可靠时，回退到既有 `BODY.PEEK[]` 完整邮件路径。
- 数据库迁移只新增，不修改既有迁移；保留现有 `part_index` 语义并新增稳定 section 字段。
- BODYSTRUCTURE 大小是传输编码后的大小；附件下载后以实际解码大小更新。

## 验证门禁

- Rust 测试覆盖 multipart/alternative、multipart/mixed、嵌套 section、附件元数据、按 section 下载和完整邮件回退。
- 前端测试覆盖紧凑附件区、单击下载并打开、loading、右键菜单和禁用的“打开方式”。
- `cargo fmt --all -- --check`。
- `cargo test --offline --locked`。
- `cargo clippy --offline --locked --all-targets -- -D warnings`。
- `pnpm test`。
- `pnpm build`。
- `git diff --check`。
- 手动验收：正文先于大附件可读；未下载附件单击显示 loading 后打开；已下载附件可打开、定位和另存为；右键菜单行为与状态正确；查看原始邮件仍可用。

## 实施结果

- 邮件头 FETCH 已同时读取 `BODYSTRUCTURE` 并事务保存附件元数据及规范化数字 section；正文只获取首选纯文本/HTML part 和实际引用的安全 CID 图片。
- 普通附件通过其 MIME section 按需下载，解码后写入既有账户隔离内容存储；打开、打开文件夹、另存为和转发/远程草稿继续复用现有安全链路。
- 无法可靠映射的复杂 multipart 附件整体回退完整 EML，网络或认证失败不会静默扩大下载范围；已下载附件的真实解码大小不会被后续 `BODYSTRUCTURE` 覆盖。
- 阅读区附件改为紧凑文件块，删除“附件：”标题和行内按钮；单击打开，未下载时显示 spinner，右键菜单提供打开、禁用的打开方式、打开文件夹和另存为。
- 删除已失去作用的“下载附件后自动打开”偏好；Vitest 只收集 `src/` 内前端测试，Updater 的 Node 测试继续独立执行。

## 自动验证

- `cargo fmt --all -- --check`：通过。
- `cargo test --offline --locked`：174 项通过。
- `cargo clippy --offline --locked --all-targets -- -D warnings`：通过。
- `pnpm test`：42 个测试文件、156 项测试通过。
- `pnpm build`：通过；仅保留既有大 chunk 提示。
- `node --test .github/scripts/generate-updater-manifests.test.mjs`：1 项通过。
- `git diff --check`：通过。

## 手动验收重点

1. 找一封带大附件的未缓存邮件，确认正文先出现，附件仍为未下载状态。
2. 单击未下载附件，确认文件块显示 spinner，完成后由系统默认应用打开；高风险扩展名仍只定位到文件夹。
3. 右键分别验证“打开”“打开文件夹”“另存为”；“打开方式”应显示但保持禁用。
4. 验证普通 multipart/alternative、带 CID 图片邮件、转发带附件邮件和远程草稿附件。
5. 验证“查看原始邮件”仍会按需取得完整 EML，复杂 MIME 邮件可回退显示。

## 手动验收

- 2026-08-14：Windows 实机验收通过。
