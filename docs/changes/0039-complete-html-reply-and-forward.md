# 0039 完整 HTML 回复与转发

日期：2026-07-21

状态：已于 2026-07-21 通过 Windows 手动验收。

## 变更

- 回复、回复全部和转发优先从账户隔离的本地原始 EML 解析 HTML part；缺失时回退到缓存的安全 HTML 或纯文本，不再把每一行转换为 `> ` 引用。
- 新增 compose 专用安全片段：保留正常邮件结构、传统表格属性、链接、图片信息和安全展示样式；继续移除脚本、事件、表单、嵌入文档、危险 URL 与 CSS 网络资源。内嵌样式表选择器被限定到原文容器，不能作用到 Composer 其他区域。
- Rust 初始草稿使用 `nextmailReply` 与 `nextmailOriginalMessage` 建立稳定边界；第九阶段场景模板位于回复区，默认签名始终位于原文之前，原始邮件头和内容保持可编辑。
- 第一次实机反馈确认 Tiptap/ProseMirror 表格 schema 会规范化不规则邮件表格、补段落与列信息并改变原始宽度。引用原文最终不再解析为表格节点，而以原始安全 HTML 作为权威内容，在 `sandbox=""`、`no-referrer` 的 iframe 中预览；普通撰写内容仍使用 Tiptap。
- 引入 MIT 许可 CodeMirror 6 HTML 编辑器，工具栏可切换完整源码与实时沙箱预览双栏。模板/签名编辑弹窗改为占满设置窗口可用高度；名称/主题字段不再参与纵向伸展，长内容由始终可见且预留稳定槽位的编辑器滚动条承载。
- 数据迁移 0013 为草稿附件增加 CID/inline 元数据。回复与转发从本地原始 MIME 缓存正文引用的内嵌图片；用户粘贴的 PNG/JPEG/GIF/WebP 经 Rust 类型、魔数与大小校验后写入既有内容寻址附件存储。HTML 保存 `cid:`，发件生成 `multipart/related`，普通附件仍保持 `multipart/mixed`。
- 编辑器不再显示图片占位卡片。CID 与粘贴图片使用缓存 data URL 预览；远程 `http(s)` 图片不在编辑器中静默请求，但安全地址仍随 HTML 保存。远程代理和无条件下载不在本批范围。
- 模板/签名查找与替换改为递归节点定位。新签名在原文前插入，切换或删除定义不会跨越原文边界。
- Composer 不为已应用签名添加左侧引用线、底色、内边距或圆角。原文沙箱按正文、表格行和可用内嵌图片估算完整展开高度并关闭内部滚动，统一由外层编辑器滚动；没有为测量高度开放脚本或 `same-origin`。
- 第二次表格实机失败已从原始 EML 定位：作者用 `nth-child()` 的 flex 比例同步表头和数据行，旧选择器白名单把函数型伪类整组删除。清洗器现只增加受限 An+B 的四种 `nth-*()` 结构伪类，并把同构的匿名发票 HTML 纳入正式语料。迁移 0014 将格式版本提升到 14 并失效旧阅读缓存；没有增加网络、脚本、iframe 或 IPC 能力。
- 官方 `@tiptap/extension-table` 3.27.3 与 CodeMirror 6 依赖均为 MIT；没有使用 Tiptap Pro、Cloud 或其他商业组件。

## 保持不变

- `In-Reply-To`、`References`、回复/转发主题前缀、Bcc 隔离、草稿 revision、远端 Drafts 替换、转发附件复制和持久化 SMTP/MIME 路径不变。
- 阅读器继续使用 ADR 0002/0008 约束的 sandbox iframe；Composer 原文与源码预览使用更严格的空 sandbox。没有开放 scripts、forms、same-origin、top-navigation、任意文件、任意网络或 Capability。
- 只新增窄的 `add_draft_inline_image` IPC；数据格式版本 13 扩展草稿内嵌图片元数据，版本 14 只失效需按新 CSS 规则重建的阅读缓存。没有进入搜索、会话、托盘、通知、POP3、OAuth 或全局优化。

## 验证

- `pnpm test`：22 个测试文件、52 项测试通过；`pnpm build` 通过，保留大 chunk 提示。
- `cargo fmt --all -- --check`、96 项 `cargo test --offline --locked`、严格 Clippy 与 `git diff --check` 通过。
- 未运行 Tauri bundle；Windows 手动验收已通过，macOS 未执行。完整记录见第十阶段 iteration。
