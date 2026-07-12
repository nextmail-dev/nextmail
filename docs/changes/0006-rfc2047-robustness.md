# 变更 0006：RFC 2047 头字段健壮性

日期：2026-07-12

## 目标

邮件头解码不得依赖某一个中文字符集特例。NextMail 按 RFC 2047 的 encoded-word 模型处理 `charset`、`B/Q` 编码和头字段上下文；MIME 正文字符集继续由独立的正文解析层处理。

## 实现决策

- `mail-parser` 继续作为唯一的 RFC 5322、RFC 2045–2049、RFC 2231、RFC 6532 和 MIME 解析器，不在 Adapter 中重复实现 encoded-word 语法。
- 启用其 `full_encoding` 特性，使库文档列出的 41 种字符集及别名全部生效，包括 UTF-7/8/16、GB2312/GBK/GB18030、Big5、ISO-8859、Windows code pages、日文和韩文编码。
- `B`/`Q`、Q 编码下划线空格、相邻 encoded-word、CRLF 折行、线性空白忽略、混合 ASCII 和地址 phrase 均直接使用库实现，并以 NextMail 回归语料锁定行为。
- 未知字符集和畸形 encoded-word 验证为安全降级：不会崩溃、不会吞掉后续 Message-ID 等头字段。
- Adapter 只把库的结构化 `Subject`、`Address`、正文和附件结果映射为领域 DTO；原始 EML 保持不变。
- 本批次不再增加数据库格式版本。此前格式 3 迁移已经重新同步使用完整字符集支持解析的邮件头和正文。

## 验证语料

- UTF-7/UTF-8 Base64 与 Q 编码。
- GB2312 Base64 标题、地址显示名和正文。
- ISO-8859-1 与 Windows-1252 字符集别名。
- 相邻、折行 encoded-word 与普通 ASCII 混排。
- 多地址以及显示名内部逗号。
- 未知字符集和畸形 Q 编码的安全降级。

## 自动验证

- `cargo test --workspace`：22 个 Rust 测试通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo fmt --all -- --check`：通过。

规范依据：[RFC 2047](https://www.rfc-editor.org/rfc/rfc2047.html)。
