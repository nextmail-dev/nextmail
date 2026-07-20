# 0035 场景规则与 Composer 集成

日期：2026-07-21

## 变更

- 新增 SQLite 数据格式版本 9 和 `composition_scene_rules`，为新建、回复、回复全部、转发保存全局或账户模板/签名组合；账户无显式规则时继承全局规则。
- 场景规则通过匿名 `account_slot_id` 隔离、使用 revision 防止陈旧覆盖，并验证引用定义的可见范围；仍被默认规则引用的模板或签名不能删除。
- 在 Rust application 增加 `sender_name`、`sender_email`、`recipient_name`、`recipient_email`、`date` 白名单校验与渲染。HTML、主题、编辑器 JSON 和纯文本分别处理，未知变量或缺失上下文返回稳定错误。
- 设置窗口新增四种默认场景规则管理和全局/账户来源标记；账户可以继承全局场景，也可以保存自己的完整场景规则。
- Composer 新增模板/签名选择。Bootstrap 只携带当前账户可见的定义摘要，选择时把当前收件人上下文交给 Rust 渲染，不在 React 执行模板代码。
- Tiptap 新增 `nextmailTemplate` 与 `nextmailSignature` 可编辑块节点，节点及 HTML 保存稳定定义 ID。切换只替换同类节点，手动删除签名后自动保存和草稿重开不会恢复。
- 四种场景只在草稿首次创建时解析默认规则；远端已有草稿不重新套用。既有回复线程头、转发附件、远端 Drafts、三格式草稿、MIME 和持久化发送路径保持不变。

## 依赖与边界

- 将现有 Tiptap 依赖链中的 `@tiptap/core` 3.27.3（MIT）声明为直接依赖，用于自定义节点；没有引入商业、Cloud 或 Pro 扩展。
- 没有修改凭据存储、Capability、HTML 阅读器清洗/sandbox、网络协议或第十阶段完整 HTML 引用范围。

## 验证

- `pnpm test`：21 个测试文件、45 项测试全部通过。
- `pnpm build`：TypeScript 与 Vite 生产构建通过；主入口仍有大于 500 kB 的既有提示。
- `cargo fmt --all -- --check` 通过。
- `cargo test --offline --locked`：66 项测试全部通过。
- `cargo clippy --offline --locked --all-targets -- -D warnings` 通过。
- `git diff --check` 通过。

未执行 Tauri bundle；保留正常的 `dist` 与 `src-tauri/target` 增量缓存。

## 手动验收

2026-07-21：用户在 Windows 实机确认场景规则、变量渲染和 Composer 模板/签名集成功能正常，本批验收通过。
