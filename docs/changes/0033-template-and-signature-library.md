# 0033 模板与签名库

日期：2026-07-20

## 变更

- 新增 SQLite 数据格式版本 8，以及 `mail_templates`、`mail_signatures` 两张富文本定义表。
- 全局定义使用空账户槽；账户定义只保存匿名 `account_slot_id`，并通过 Repository 查询条件保持账户隔离。
- 新增模板/签名 DTO、application 输入规范化、`CompositionDefinitionRepository` 和八个薄 Tauri Command；React 继续只通过 `src/app/api.ts` 访问稳定接口。
- 定义保存 Tiptap editor JSON、HTML 与纯文本，使用 revision 防止陈旧更新或删除覆盖新内容，列表按名称稳定排序。
- 设置窗口“写信”分类新增全局/账户范围选择、模板和签名列表、富文本新建/编辑、二次确认删除和中英文文案。
- 富文本编辑器允许在定义管理场景隐藏临时基础签名按钮，并支持独立可访问名称；写信窗口现有行为不变。

## 边界与后续

- 本批只完成模板与签名库，不执行变量替换，不配置场景默认规则，也不把定义插入 Composer。
- 没有修改草稿 schema、回复/转发组合、远端 Drafts、MIME、SMTP、附件、HTML 阅读器、Capability 或凭据边界。
- 第九阶段第二批将在第一批验收后实现变量、全局/账户默认规则、稳定签名节点和新建/回复/回复全部/转发集成。

## 验证

- `pnpm test`：20 个测试文件、42 项测试全部通过。
- `pnpm build`：TypeScript 与 Vite 生产构建通过；主入口仍有大于 500 kB 的既有提示。
- `cargo fmt --all -- --check` 通过。
- `cargo test --offline --locked`：62 项测试全部通过。
- `cargo clippy --offline --locked --all-targets -- -D warnings` 通过。
- `git diff --check` 通过。

未执行 Tauri bundle；保留正常的 `dist` 与 `src-tauri/target` 增量缓存。

## 手动验收

2026-07-20：用户在 Windows 实机确认全局/账户模板与签名库功能正常，第一批验收通过。
