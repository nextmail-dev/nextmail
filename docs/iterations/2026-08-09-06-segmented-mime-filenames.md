# 分段 MIME 附件文件名兼容

日期：2026-08-09

## 状态

已验收。

## 范围

- 修复部分非标准客户端把附件 `name*0` / `name*1`、`filename*0` / `filename*1` 分段拼接后才形成完整 RFC 2047 encoded-word 时的文件名解析。
- 在 Rust MIME 解析边界兼容连续、从 0 开始的 RFC 2231 continuation 参数：先按序拼接，再执行既有 RFC 2047 / 字符集解码。
- 同时覆盖 `Content-Disposition` 的 `filename` 与 `Content-Type` 的 `name` 回退，不改变正文、CID 或附件字节处理。
- 增加用户所给真实形态 EML 回归，确保附件摘要向持久化与后续打开/另存为链路提供完整文件名。

## 非目标

- 不放宽附件路径、内容类型、文件魔数、大小预算、账户隔离或系统打开安全边界。
- 不修改第三方 `mail-parser`，不为任意缺号、乱序或重复 continuation 猜测内容。
- 不修改数据库结构、产品版本、发件 MIME 生成或前端布局。
- 不提交、推送、创建 tag 或发布。

## 验证门禁

- 示例中的两段 Base64 encoded-word 解码为完整中文 `.xlsx` 文件名，不能截断、保留 encoded-word 包装或产生路径片段。
- 标准单段 `filename`、标准 RFC 2231 百分号编码、普通 RFC 2047 文件名及缺失 continuation 段保持原有安全降级。
- 文件名规范化后仍不能携带路径穿越、绝对路径、控制字符或内部路径。
- 执行针对性 MIME 测试、`cargo fmt --all -- --check`、`cargo test --offline --locked`、`cargo clippy --offline --locked --all-targets -- -D warnings` 与 `git diff --check`；不运行 Tauri bundle。

## 实施结果

- 根因确认：`mail-parser` 会合并 `filename*0` / `filename*1` 与 `name*0` / `name*1`，但单段尚不完整时无法执行 RFC 2047 解码，合并后也不会再次解码，因此向上层返回完整 encoded-word 原文。
- 在共享附件名边界增加严格兼容解码：仅当合并结果是独占整个值的完整 RFC 2047 encoded-word 时补做一次解码；残缺编码、带尾随文本的值和普通文件名保持既有结果。
- 普通附件摘要与 Composer 导入的内嵌附件统一使用该边界；`Content-Disposition filename` 仍优先于 `Content-Type name`。
- 新增 `testdata/mail-rendering/segmented-rfc2047-attachment-name.eml`，覆盖用户提供的分段形态，并补充 `Content-Type name` 回退、标准 RFC 2231 百分号分段和严格降级测试。

## 验证结果

- `cargo fmt --all -- --check`：通过。
- `cargo test --offline --locked`：通过，168 项测试全部成功。
- `cargo clippy --offline --locked --all-targets -- -D warnings`：通过。
- `git diff --check`：通过（仅有仓库现存行尾转换提示）。

## 验收结果

- 2026-08-09：用户确认验收通过，本计划随 `v0.3.0` 发布。
