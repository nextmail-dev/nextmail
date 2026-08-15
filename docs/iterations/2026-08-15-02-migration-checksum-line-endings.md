# 迁移校验和行尾不一致修复

状态：等待手动验收

## 目标

修复 Windows 本机 dev 构建启动即报 `data_directory.database_migration_failed`、无法打开既有数据库的问题，并消除本机构建与 CI 发布构建对同一数据库的校验和不兼容。

## 背景与诊断

sqlx 在每次打开仓库时运行 `MIGRATOR`，校验 `_sqlx_migrations` 中已记录迁移的 SHA-384 校验和；校验和按编译时迁移文件**原始字节**（含行尾）计算并经 `include_str!` 嵌入二进制。仓库未固定行尾，本机 `core.autocrlf=true` 使检出内容为 CRLF，而开发会话工具直接写入的文件是 LF；CI（Linux）构建始终是 LF。于是同一份提交在不同机器上编译出不同校验和。

实机证据：本机安装版 0.6.5 为本地构建（2026-08-15 00:55，内嵌 CRLF 校验和），已将数据库 `_sqlx_migrations` 全部 29 行写成 CRLF 基准；随后 dev 树中 0027–0029 为 LF，`pnpm tauri dev` 立即校验失败（错误为确定性 `VersionMismatch`，日志中每次打开仓库都在毫秒级失败，无 15 秒锁等待）。该问题与同日前端渲染性能修复无关。

## 范围

- 新增 `.gitattributes`，将 `src-tauri/migrations/*.sql` 固定为 `text eol=lf`，所有平台检出与 CI 字节一致。
- 迁移目录按新属性重新检出，29 个文件全部为 LF（`git status` 保持干净）。
- 修正用户数据库 `_sqlx_migrations` 全部 29 行校验和为 LF 值；修正前以 SQLite backup API 全量备份至 `E:\NextMail-Data\content.sqlite.backup-2026-08-15-lf-fix`。
- 以规范化后的树执行 `pnpm tauri build --no-bundle`，替换本机安装目录 exe；旧 CRLF 版本保留为 `nextmail.exe.crlf-backup`。

## 非目标

- 不修改任何迁移 SQL 内容；不新增迁移。
- 不改变数据库 schema、数据或应用行为。
- 不处理本机 `core.autocrlf` 全局配置本身；`.gitattributes` 只对迁移文件强制 LF。

## 验证门禁

- 29 个迁移文件全部 LF。
- 数据库 29 行校验和 = 仓库文件计算值 = 新 exe 内嵌值，三方一致。
- `cargo test --offline --locked`。
- `pnpm tauri dev` 可正常打开既有数据库并同步。
- 安装版启动正常，日志无 `data_directory.database_migration_failed`。

## 实施结果

- `.gitattributes` 已添加并注明原因；迁移目录重新检出为 LF。
- 数据库 29 行校验和全部更新为 LF 值并复核一致；备份保留在数据目录。
- 新 release exe（仅内嵌 LF 校验和）已替换 `C:\Users\TaurusXin\AppData\Local\NextMail\nextmail.exe`，旧版本保留为 `nextmail.exe.crlf-backup`。

## 自动验证

- `cargo test --offline --locked`：181 项通过。
- 校验和三方一致性检查：29/29 一致，无问题项。

## 手动验收重点

1. `pnpm tauri dev` 启动后正常进入主界面，账户列表、邮件同步与读取正常，日志不再出现 `data_directory.database_migration_failed`。
2. 从开始菜单/快捷方式启动安装版 NextMail，确认可正常打开同一数据库。
3. 验收通过后可删除 `content.sqlite.backup-2026-08-15-lf-fix` 与 `nextmail.exe.crlf-backup`。
