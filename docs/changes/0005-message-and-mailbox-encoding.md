# 变更 0005：邮件与文件夹字符编码

日期：2026-07-12

## 问题

- `mail-parser` 默认未启用完整多字节字符集，GB2312/GBK 等邮件标题、地址显示名和正文被按 UTF-8 有损转换，产生 `�`。
- IMAP 文件夹名直接保存了服务器线缆格式，中文等非 ASCII 名称仍显示为 modified UTF-7（例如 `&...-`）。
- 标准文件夹直接显示服务端英文名称，没有跟随 NextMail 界面语言。

## 修复

- 启用 `mail-parser/full_encoding`，按 MIME/RFC 2047 声明自动解码 GB2312、GBK、GB18030、Big5、Shift-JIS、EUC-KR 和其他受支持字符集；Base64 与 quoted-printable 传输编码继续由 MIME Parser 解码。
- 增加 IMAP modified UTF-7 解码器；数据库分别保存远端原名和 Unicode 显示名，选择远端文件夹时仍使用未经修改的线缆名称。
- 根据 SPECIAL-USE 属性及常见标准名称识别 Inbox、Sent、Drafts、Trash、Junk 和 Archive；界面按当前语言显示固定名称，其他文件夹显示解码后的用户名称。
- 数据库迁移到内部格式 3：保留原始 EML 和附件文件，重置摘要增量游标并清除旧的已解析正文，使启动后的只读同步自动重建乱码数据。

## 验证

- 增加 GB2312 RFC 2047 标题、发件人和 Base64 正文解码测试。
- 增加 RFC 3501 modified UTF-7 文件夹与标准文件夹角色测试。
- `cargo test --workspace`：19 个测试通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `pnpm test`：4 个测试通过。
- `pnpm build`：通过。
- `pnpm tauri build --debug --no-bundle`：通过。
- Windows 实际界面检查：标准文件夹已显示为中文，自定义 modified UTF-7 文件夹已恢复为 Unicode，GB2312 邮件的标题、收件人和正文显示正常。
