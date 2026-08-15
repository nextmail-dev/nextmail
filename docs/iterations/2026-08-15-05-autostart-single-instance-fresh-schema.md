# 开机自动启动、单实例与迁移重置

状态：已通过手动验收（v0.7.0 发布）

## 目标

1. 删除上一轮的迁移校验和修复（`migration_repair` 及相关文档），因为只有开发者本机仍运行旧版本，不需要过渡兼容；删除不在后续更新日志中提及。
2. 迁移重置：从本版本起不再支持从旧版本数据库升级。删除 29 个历史迁移文件，改为单一基线迁移 `0001_bootstrap.sql`（内容为当前 schema 的完整重建，schema 本身不变）；旧库打开将按既有 `data_directory.database_migration_failed` 路径失败，用户需重建数据目录。
3. 设置-通用新增“开机自动启动”开关，默认不启用；状态以系统机制（Windows 注册表 Run 键 / macOS LaunchAgent）为准，不进入桌面偏好 JSON。
4. Windows 下重复启动检测：同一应用标识只能存在一个进程，再次点击图标时显示并聚焦既有主窗口（从托盘隐藏状态恢复）。

## 背景

- 上一轮为兼容 v0.6.5 及更早的 Windows 库加了启动时校验和修复；既然决定不再支持旧库升级，该修复随之失去意义。
- 目前应用可被重复启动出多个实例，共享同一数据目录与日志；需要单实例约束并在二次启动时聚焦既有主窗口。
- 开机自启是用户明确要求的设置项；默认不启用，不做开机后窗口状态的额外定制。

## 范围

- 删除 `storage/migration_repair`、`docs/iterations/2026-08-15-04-*`、README 索引行与 `docs/project.md` 中的修复说明；恢复 `repository.rs` 与 `storage/mod.rs` 原状。
- 删除 `src-tauri/migrations/` 下 29 个历史文件，新增 `0001_bootstrap.sql`（按序拼接既有 29 个迁移的 SQL，含 FTS5 触发器等完整 schema）；`.gitattributes` 的 LF 约束保留。
- Rust 新增官方插件 `tauri-plugin-autostart` 与 `tauri-plugin-single-instance`；新增薄命令 `get_autostart_enabled` / `set_autostart_enabled`（经 `api.ts` 暴露）；单实例回调显示/聚焦 `main` 窗口。
- 设置-通用新增自启动复选框（查询 + 变更，失败回滚显示错误），文案双语。
- `docs/project.md` 同步：schema 元数据版本改为 1（单一基线）、不兼容旧库的说明、功能清单与窗口约束更新。

## 非目标

- 不修改任何 schema 内容、不新增业务表或字段。
- 不做旧库自动检测/清理/迁移；旧库用户自行重建数据目录。
- 不改桌面偏好 JSON 结构；自启动状态只存系统机制。
- 不处理 macOS/Linux 的开机自启验收（按项目平台边界，Windows 实机验收）。

## 验证门禁

- `cargo fmt --all -- --check`、`cargo clippy --offline --locked --all-targets -- -D warnings`、`cargo test --offline --locked`（Repository 测试在临时库上执行单一基线迁移，验证 schema 完整重建）。
- `pnpm test`、`pnpm build`、`git diff --check`。
- Windows 实机：新数据目录首次启动成功进入主界面；设置-通用自启动开关开/关后注册表 Run 键同步变化，默认不存在；应用运行时再次点击图标，既有主窗口显示并聚焦；托盘隐藏状态下同样可恢复。

## 实施结果

- 迁移重置完成：`migrations/` 仅剩 `0001_bootstrap.sql`（29 个历史文件内容按序拼接，schema 与原 0029 一致）；`migration_repair` 及相关文档已删除，`repository.rs`/`storage/mod.rs` 恢复原状。
- Repository 中 10 处直接 `include_str!` 历史迁移文件的测试改为内联 SQL 字面量，行为不变（继续锁定各历史迁移的局部行为）。
- 新增 `tauri-plugin-autostart@2.5.1`（`init(MacosLauncher::LaunchAgent, None)`）与 `tauri-plugin-single-instance@2.4.3`（回调显示/还原/聚焦 `main` 窗口）。
- 新增命令 `get_autostart_enabled` / `set_autostart_enabled`（薄命令经 `api.ts` 暴露为 `getAutostartEnabled` / `setAutostartEnabled`），错误码 `autostart.state_read_failed` / `autostart.update_failed`。
- 设置-通用新增“开机自动启动”复选框：TanStack Query 查询 + 乐观更新 + 失败回滚，读取失败时禁用；分组“启动”与错误文案双语已补齐。
- 带说明文字的复选框交互区改为整行宽度：`Checkbox` 描述态由 `w-fit` 改为 `w-full`，hover/焦点背景覆盖整行而非仅说明文字宽度；设置、账户管理、模板定义等全部描述型复选框统一生效，两处断言同步更新。
- 初始化向导右侧内容列增加标题栏高度补偿：向导内容与覆盖式滚动条此前伸入固定标题栏下方被遮挡，现包一层 `pt-[var(--titlebar-height)]` 的容器，内容与滚动条轨道都从标题栏之下开始；新增布局回归测试。
- 同步开始时先预创建整个文件夹树：`LIST` 返回全部文件夹后，用新增的 `ensure_mailbox`（只插入缺失行、绝不覆盖既有同步元数据）一次建齐所有文件夹并发布变更事件，侧栏立即显示目录结构，随后再逐个文件夹同步邮件；后续同步该通道为空操作。
- `docs/project.md`：schema 元数据版本 1 与不支持旧库升级的说明、功能清单、单实例窗口约束已更新。

## 自动验证

- `cargo fmt --all -- --check`、`cargo clippy --offline --locked --all-targets -- -D warnings` 通过。
- `cargo test --offline --locked`：188 项通过。
- `pnpm test`：44 个测试文件、164 项通过。
- `pnpm build`（含 `tsc`）：通过；仅保留既有大 chunk 提示。
- `git diff --check`：通过。

## 手动验收重点

1. 删除（或移走）旧数据目录后首次启动：正常进入主界面，数据库重建完成，同步与阅读正常。
2. 设置-通用出现“开机自动启动”复选框且默认关闭；开启后注册表 `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` 出现 NextMail 项，关闭后消失；重启登录系统后应用自动启动。
3. 应用运行（含主窗口隐藏到托盘）时再次双击桌面图标/快捷方式：不出现第二个进程，既有主窗口显示并聚焦。
4. 旧数据目录场景：直接打开旧库应提示既有迁移失败错误，不会自动删除任何数据。

