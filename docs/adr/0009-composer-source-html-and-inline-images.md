# ADR 0009：Composer 原始 HTML 与内嵌图片边界

状态：已接受

日期：2026-07-21

## 背景

第十阶段第四批最初把清洗后的引用原文导入 Tiptap/ProseMirror。真实邮件实机验收发现，ProseMirror 表格模型必须把内容规范化为矩形 schema，会补段落、列信息和单元格，从而改变不规则或精细排版邮件的原始几何。图片占位节点也无法满足粘贴、回复、转发后可靠保留正文图片的需求。

ADR 0002/0008 已规定不可信邮件 HTML 不能进入具备 Tauri IPC 的主 React DOM，也不能通过 `allow-same-origin`、脚本或通用网络权限换取显示效果。邮件标准则使用 `multipart/related` 和 `cid:` 关联 HTML 与内嵌 MIME part；本机文件路径不是可发送、可迁移或可跨客户端解析的邮件资源标识。

## 决策

### 1. 原始 HTML 是引用原文的权威表示

- 回复/转发的用户撰写区、模板和签名继续使用 Tiptap；引用原文以稳定 `nextmailOriginalMessage` 原子节点保存 `sourceHtml`/`sourcePlainText`，不再解析为 ProseMirror 表格或图片树。
- 富文本视图把原文放入 `sandbox=""`、`referrerpolicy="no-referrer"` 的 `srcdoc` iframe。CSP 只允许行内样式和 data URL 图片；不允许 scripts、forms、same-origin、top-navigation、弹窗、任意文件或任意网络。
- CodeMirror 6 提供完整 HTML 源码与实时沙箱预览双栏。源码切回富文本时重新识别 NextMail 的稳定节点边界，但原文内部 HTML 不进入 ProseMirror schema。
- HTML 源码在浏览器中只进入隔离预览；保存模板、签名或草稿时，Rust 对完整 HTML 及编辑器 JSON 中的原文属性再次执行主动内容、URL 和 CSS 清洗。未经处理的源码不得直接进入发件 MIME。

### 2. 图片使用受管内容存储和 CID

- 本地原始 EML 中实际被 HTML `cid:` 引用的 PNG、JPEG、GIF、WebP part，以及用户主动粘贴的同类图片，写入现有 SHA-256 内容寻址 `attachments/` 存储。React 不获得文件路径或内容哈希。
- `draft_attachments` 记录 `content_id` 与 `is_inline`；前端只获得不透明附件 ID、CID 和用于当前编辑会话的 data URL 预览。预览数据不写回 HTML，持久化 HTML 始终使用 `cid:`。
- 粘贴图片通过 `src/app/api.ts` 的单一窄 Command 进入 Rust，校验允许的 MIME、文件魔数、25MB 单项上限、100MB 草稿总上限和账户槽/草稿可编辑归属。
- 发件 MIME 使用 `multipart/alternative` 包含纯文本和 HTML；HTML 与 CID part 组成 `multipart/related`；普通附件存在时再包入外层 `multipart/mixed`。持久化 send job 继续只引用不可变 MIME 哈希。

### 3. 不静默缓存远程图片

- `http(s)` 图片可能包含打开跟踪。进入 Composer 不构成用户授权下载；未缓存远程图片在编辑器与源码预览中隐藏，不显示占位卡片，也不发起请求。
- 通过安全清洗的远程地址继续保存在 HTML 中，收件客户端可按自身策略处理。若未来要在回复时主动下载并转为 CID，必须复用明确的远程内容许可并单独设计失败、大小、重定向、DNS/SSRF 与缓存策略。

## 影响

- 复杂原始表格不再因编辑器 schema 被重排；修改原文内部结构使用 HTML 源码面板，不能在 Tiptap 中逐单元格直接编辑。
- CID 与粘贴图片可离线重开并可靠随草稿/发件 MIME 保存；数据格式版本 13 新增草稿 inline 元数据。图片字节仍位于可迁移内容存储，账户配置和秘密边界不变。
- Composer 原文/源码预览比阅读器更严格：使用空 sandbox，不承担点击外部链接功能。阅读器继续遵循 ADR 0008 的 `allow-popups` + 宿主复验系统打开路径。
- CodeMirror 6 及使用的模块为 MIT 许可；没有引入 Tiptap Pro/Cloud 或其他商业组件。

## 验证门禁

- 回归必须证明原始表格 HTML/宽度属性在 Tiptap 保存更新后仍逐字保留于原文节点，并且原文 iframe 不包含任何 sandbox allow token。
- Rust 必须覆盖 CID MIME 导入、图片字节缓存、账户槽隔离、`multipart/related` 构建和 HTML/JSON 二次清洗。
- 前端必须覆盖 CID data URL 替换、远程图片不请求/无占位、HTML 源码双栏和模板/签名编辑高度。
- Windows 需实机验证真实复杂表格、回复/转发内嵌图、剪贴板图片、保存重开和实际收件端显示；macOS 未执行前不得记录通过。
