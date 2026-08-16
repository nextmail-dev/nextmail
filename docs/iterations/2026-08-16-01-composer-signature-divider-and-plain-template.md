# Composer 编辑器调整:签名分隔线、模板原样插入与行距

状态：已验收

## 范围

1. 新建邮件窗口按“自动插入默认签名”组装初始内容时,Rust `assemble_composition_content` 的非回复路径在签名上方补 `nextmailSignatureDivider`(`editor_json` 与 `html` 同步,纯文本与回复路径一致使用 `----------------` 分隔),与既有回复/转发路径及前端手动选择签名的行为对齐。
2. 移除 Composer 编辑器中 `.nextmail-composition-template` 的边框、背景与内边距样式,模板内容按原版原样插入,不套用任何视觉包装;模板节点与 `data-nextmail-template-id` 身份标识保持不变。
3. Composer 富文本编辑器 `.nextmail-editor-content` 默认 `line-height` 从 1.65 收紧到 1.5。

## 非目标

- 不改变模板/签名渲染、变量替换、场景规则和收件人覆盖逻辑。
- 不改变签名在回复/转发路径的既有分隔行为。
- 不调整发件 MIME 生成与既有 `hr` 白名单。

## 验证

- `src-tauri` 内 `cargo fmt --all -- --check`、`cargo test --offline --locked`、`cargo clippy --offline --locked --all-targets -- -D warnings`。
- 前端 `pnpm test`、`pnpm build`。
- `git diff --check`。

## 实施与验收记录

- `src-tauri/src/application/composition_definitions.rs`:`assemble_composition_content` 非回复路径在签名节点前插入 `nextmailSignatureDivider`,`html` 同步插入带既有内联样式的 `<hr data-nextmail-signature-divider>`,纯文本在签名前加入与回复路径一致的 `----------------` 分隔;更新既有组装测试断言。
- `src/styles/composer.css`:移除 `.nextmail-composition-template` 的边框、背景、圆角与内边距,与签名节点同样归零为透明容器;模板内容自身块级间距按正文原样保留(`:last-child` 底边距修剪仅保留给签名)。
- `src/styles/composer.css`:`.nextmail-editor-content` 行高 1.65 -> 1.5。
- 验证:Rust `cargo fmt --all -- --check`、`cargo test --offline --locked`(188 passed)、`cargo clippy --offline --locked --all-targets -- -D warnings` 全部通过;前端 `pnpm test`(164 passed)、`pnpm build` 通过(既有大于 500 kB chunk 警告不变);`git diff --check` 干净。
- 待用户 Windows 实机验收:新写邮件自动插入默认签名时签名上方出现细分隔线;插入模板时正文区无任何背景/边框包装,模板内容原样呈现;编辑器默认行距收紧。
- 2026-08-16 用户 Windows 实机验收通过。
