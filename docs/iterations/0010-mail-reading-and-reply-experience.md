# 第十阶段：邮件阅读与回复体验优化

状态：规划中。依赖第九阶段的签名节点与默认规则，尚未实施。

## 阶段定位

该阶段集中改善收到邮件的 HTML/CSS 保真度、外部链接交互、深色模式，以及回复/转发时的原始内容完整度。目标是让常见营销邮件、通知邮件和富文本往来尽量保持原排版，同时继续把邮件视为不可信输入。

参考资料：

- [The Complete Guide to Email Client Rendering Differences in 2026](https://dev.to/mailpeek/the-complete-guide-to-email-client-rendering-differences-in-2026-243f)
- [Gmail 官方 CSS 支持表](https://developers.google.com/workspace/gmail/design/css)
- [WHATWG HTML iframe sandbox 规范](https://html.spec.whatwg.org/multipage/iframe-embed-object.html)

这些资料说明邮件客户端对 `<style>`、媒体查询、表格和深色模式的处理差异很大，也说明 sandbox 能把不可信文档置于不透明 origin 并禁用脚本、表单和顶层导航。兼容性资料不等同于桌面 WebView 的威胁模型，因此“移除 iframe”不能由渲染效果单独决定。

## 当前事实

- Rust `sanitize_mail_html` 当前整段删除 `<style>`、所有链接 `href`、表单、脚本、嵌入文档和 CSS 资源 URL，只保留白名单内的行内样式。
- 阅读器使用 `sandbox=""` 的 `srcdoc` iframe，不允许 scripts、forms、same-origin、popups 或 top-navigation；远程图片由文档 CSP 默认阻止。
- 深色模式对 `html/body` 注入 `!important` 背景和前景色，可能覆盖邮件原有页面级配色。
- 回复/回复全部和转发只读取纯文本；回复逐行加 `> `，转发生成纯文本头，随后转义成简单 HTML。
- 当前 Tiptap schema 不足以无损导入复杂表格、邮件内样式和其他完整 HTML 结构。

## 目标一：高保真且安全的 HTML 阅读

### 样式保留

- 不再无条件删除邮件自带的 `<style>`；引入真正的 CSS 解析与过滤步骤，保留安全的选择器、表格布局、字体、颜色、尺寸、间距和受控媒体查询。
- 保留现有安全底线：脚本、事件属性、表单、嵌入文档、危险 URL、外部样式表、`@import`、可执行内容和任意本地资源继续禁止。
- CSS 中可能发起网络请求的 `url()`、字体、背景和列表资源统一进入远程资源策略；不能绕过“远程图片默认阻止”或获得任意文件/网络能力。
- 原始 EML 继续作为可重建来源；清洗策略升级通过新增迁移失效旧正文缓存，不修改已发布迁移。

“保留样式”在本阶段指尽量保留不会执行代码、导航、读取本机数据或偷偷发起资源请求的展示语义，不表示把未经处理的任意 CSS/HTML 直接交给 WebView。

### 深色模式

- 对没有显式背景/文字样式的邮件，让正文继承 NextMail 深灰背景和浅灰文字，并提供合适的链接、分隔线和表格默认色。
- 对已经明确设置颜色和背景的邮件优先保留作者样式，移除当前对所有 `html/body` 的 blanket `!important` 覆盖。
- 评估邮件声明的 `color-scheme`、`supported-color-schemes` 和 `prefers-color-scheme`，只在安全 CSS 子集内生效。
- 使用无样式纯文本、浅色营销邮件、原生深色邮件、混合背景表格和低对比度语料做浅色/深色回归。

### sandbox 决策门禁

- 默认方案仍保留 sandbox iframe，并评估在不开放 same-origin、forms、top-navigation 和任意脚本的前提下提升样式保真。
- 可进行隔离原型，比较 sandbox `srcdoc`、专用隔离 WebView 或其他不与应用 DOM/IPC 同源的方案；不能把不可信邮件直接插入具备 Tauri IPC 的主 React DOM。
- 如果最终方案需要移除 iframe、允许受控脚本或改变 origin/IPC 隔离，必须先提交新的 ADR 取代或修订 ADR 0002，列出威胁模型、Capability、CSP、导航和恶意语料结果，再由用户确认。

## 目标二：受控外部链接

- Rust 保留并规范化通过校验的 `http`、`https` 和 `mailto` 链接，其他 scheme、控制字符、混淆 URL 和本地路径继续移除。
- 点击链接时由 NextMail 显示中英文确认，明确提示即将离开 NextMail、目标地址不受 NextMail 控制，用户应自行确认安全。
- 只有用户确认后，才把已校验 URL 交给 Rust 系统打开用例；邮件文档本身不能直接导航顶层窗口或获得通用 opener 权限。
- 具体 iframe 到宿主的点击桥接方式与 sandbox 决策一起验证；公开事件只传不透明 link ID，前端不接收任意命令或内部路径。

## 目标三：回复/转发保留完整可编辑内容

### 内容模型

- 回复/回复全部/转发优先使用原始 MIME HTML part 作为来源，不再把正文降级为逐行 `> ` 的纯文本再重新生成 HTML。
- “不清洗原始内容”按产品体验解释为不丢失正常排版、表格、图片占位、链接和安全样式；脚本、事件、表单、导航、任意资源请求等主动能力仍必须移除。
- 未经处理的原始 HTML 不得直接进入 Composer WebView。建立 compose 专用的高保真非主动内容导入器，并保留 raw EML 作为来源和故障恢复依据。
- HTML 邮件没有可用 HTML part 时，使用原始纯文本和转发头生成结构化编辑器节点，但不再为每一行添加可见的 `> `。

### 富文本编辑器

- 扩展 Tiptap/ProseMirror schema，使常见邮件表格、列表、blockquote、链接、图片占位和允许的行内样式能够导入、编辑并 round-trip 到 HTML/纯文本。
- 只采用许可证确认过的开源扩展；商业或 Pro 扩展必须先询问。
- 为引用原文、用户回复和签名建立稳定节点边界，防止保存/重开或切换签名时破坏原文。
- 编辑区默认顺序为：顶部空白回复区域、一个空行、默认签名节点、原始邮件头与完整可编辑引用内容。转发继续保留原附件关联。
- 第九阶段的签名规则是该布局的唯一来源，避免 RichTextEditor 再维护一套临时身份签名。

### MIME 与线程语义

- `In-Reply-To`、`References`、回复/转发主题前缀、Bcc 隔离和持久化 MIME 发件语义保持不变。
- HTML 与纯文本 alternative 从同一个编辑器文档生成；纯文本引用保留完整内容和原始邮件头，但不强制逐行 `> `。
- 草稿 revision、远端 Drafts 替换、附件内容寻址和发送任务恢复保持现有事务边界。

## 计划批次

1. **语料与安全决策**：建立真实邮件/恶意 HTML/CSS/链接/深色模式语料，完成渲染隔离原型和 ADR 门禁。
2. **阅读器保真与深色模式**：CSS 解析过滤、缓存失效、主题默认样式和视觉回归。
3. **受控外链**：link ID、确认 UI、Rust 系统打开边界和恶意 scheme 测试。
4. **完整回复/转发**：高保真内容导入、编辑器 schema、签名布局、三格式 round-trip 和附件/线程回归。

每个批次单独完成自动验证并交付手动验收；用户确认前不进入下一批。

## 非目标

- 不允许邮件脚本、事件处理器、表单提交、嵌入页面、same-origin、顶层导航、任意网络或任意文件权限。
- 不实现远程内容代理、CID/内联附件完整协议之外的通用浏览器能力；若这些成为保真前提，需要单独扩展计划。
- 不实现搜索、会话、托盘、通知、POP3 或 OAuth。
- 不借本阶段进行全局性能、工具链、CI、发布或无关 UI 重构。

## 自动验收

- Rust：HTML/CSS/URL 恶意语料、样式保留、远程资源阻断、缓存失效和账户隔离。
- 前端：sandbox/CSP、浅色/深色默认、外链确认、编辑器导入/编辑/保存、回复/转发布局和签名节点。
- 完整运行 `pnpm test`、`pnpm build`、Rust fmt/test/Clippy 和 `git diff --check`，不执行 Tauri bundle。

## 手动验收

- Windows 真实邮件覆盖无样式正文、复杂表格、营销邮件、浅/深色邮件、远程图片和多种外链。
- 链接必须先显示准确目标与离开 NextMail 警告，取消时不打开，确认后只由系统浏览器/邮件程序处理。
- 回复、回复全部和转发重开草稿后仍保留完整可编辑原文、顶部空白回复区、默认签名、线程头和附件。
- macOS 只有在真实设备执行后才记录通过。
