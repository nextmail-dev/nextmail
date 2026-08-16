# 阅读区附件未下载云朵标记与另存为时序

状态：已验收

## 范围

- 阅读区附件文件块在附件尚未下载(`availability` 非 `available`)时,于大小文本右侧同行显示 12px 云朵图标,附带“尚未下载 / Not downloaded yet”无障碍标签;下载完成后随可用状态自动消失。
- 附件“另存为”改为先弹出保存对话框选择目标位置,确认后才按需下载并写入;取消对话框不再触发任何下载。
- 附件转圈只反映实际内容传输:Rust 在附件内容真正开始获取时发布 `attachment-download-started { accountId, attachmentId }` 窄事件;保存对话框打开期间不转圈,确认目标并开始下载后才转圈,下载结束随 mutation 收尾清除。打开/定位路径下载立即开始,行为不变。
- 中英文新增文案 `mail.attachmentNotDownloaded`。

## 非目标

- 不改变附件单击打开、按需下载流程、右键菜单与文件块其余布局;不改变打开/定位的既有下载行为。

## 验证

- `pnpm test`(165 passed)、`pnpm build`、`src-tauri` 内 `cargo fmt --all -- --check`、`cargo test --offline --locked`(188 passed)、`cargo clippy --offline --locked --all-targets -- -D warnings`、`git diff --check`。

## 实施与验收记录

- `src/features/mail/MessageAttachment.tsx`:大小与云朵图标改为同一 flex 行(`Inline` + `gap-1`),修复图标换行到下一行的问题;`Text` 导入随之移除。
- `src/features/mail/MessageAttachment.test.tsx`:新增用例覆盖未下载显示、下载后消失。
- `src-tauri/src/mail_runtime/content.rs`:`save_message_attachment_as` 先用 `attachment_summary` 取元数据(含账户所有权校验)并以清洗后的文件名作为对话框建议名,用户确认目标后才调用 `prepare_message_attachment` 下载/物化缓存并复制到目标位置;取消返回 `false` 且零网络请求。`request_attachment` 在确认需要获取内容后发布 `attachment-download-started` 事件(`AttachmentDownloadStartedEvent` 定义于 `runtime_support.rs`,仅携带账户与附件 ID)。
- `src/features/mail/MessageViewer.tsx`:监听 `attachment-download-started` 维护当前账户下载中附件集合,mutation 结束时清除,并向 `MessageAttachment` 传入 `downloading`。
- `src/features/mail/MessageAttachment.tsx`:新增必填 `downloading` 属性;转圈条件改为 `downloading || ((opening || revealing) && !available)`,`saving` 单独存在(仅对话框阶段)不再显示 spinner;按钮禁用条件保持覆盖全部进行中操作。
- `src/features/mail/MessageAttachment.test.tsx`:新增用例覆盖对话框阶段不转圈、下载开始后转圈;`MessageViewer.test.tsx` 补 `@tauri-apps/api/event` mock。
- 验证:`pnpm test`(166 passed)、`pnpm build`;Rust 188 passed,fmt/clippy 干净;`git diff --check` 干净。
- 待用户 Windows 实机验收:未下载附件大小右侧同行显示云朵;另存为先弹保存对话框且期间不转圈,确认后开始下载才转圈,取消不产生下载。
- 2026-08-16 用户 Windows 实机验收通过。
