# 第十阶段：邮件阅读与回复体验优化

状态：实施中。前三批均已于 2026-07-21 完成 Windows 手动验收；第三批第一次自定义协议实现失败后，已按反馈改为系统直接打开并补充传统邮件布局保真修正。下一批为“完整回复/转发”，尚未开始。依赖第九阶段已验收的签名节点与默认规则。

## 阶段定位

该阶段集中改善收到邮件的 HTML/CSS 保真度、外部链接交互、深色模式，以及回复/转发时的原始内容完整度。目标是让常见营销邮件、通知邮件和富文本往来尽量保持原排版，同时继续把邮件视为不可信输入。

参考资料：

- [The Complete Guide to Email Client Rendering Differences in 2026](https://dev.to/mailpeek/the-complete-guide-to-email-client-rendering-differences-in-2026-243f)
- [Gmail 官方 CSS 支持表](https://developers.google.com/workspace/gmail/design/css)
- [WHATWG HTML iframe sandbox 规范](https://html.spec.whatwg.org/multipage/iframe-embed-object.html)

这些资料说明邮件客户端对 `<style>`、媒体查询、表格和深色模式的处理差异很大，也说明 sandbox 能把不可信文档置于不透明 origin 并禁用脚本、表单和顶层导航。兼容性资料不等同于桌面 WebView 的威胁模型，因此“移除 iframe”不能由渲染效果单独决定。

## 当前事实

- Rust `sanitize_mail_html` 已通过 CSS parser 保留安全 `<style>`/行内展示样式，并直接保留通过校验的 `http`、`https`、`mailto` 目标；脚本、表单、嵌入文档、危险 scheme 和 CSS 网络资源继续移除。
- 阅读器使用不透明 origin 的 `srcdoc` iframe，sandbox 只有供宿主拦截用户链接点击的 `allow-popups`，不允许 scripts、forms、same-origin 或 top-navigation；远程图片由文档 CSP 默认阻止。
- 深色模式已使用不带 `!important` 的浅/深色兜底，邮件作者明确的页面、类和行内配色按正常层叠优先。
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
- 点击安全链接时不显示 NextMail 确认框，直接交给系统默认浏览器或邮件程序；外部网页不能在 NextMail 内创建或加载。
- 邮件链接固定请求新窗口。主 WebView 回调再次验证目标后调用 Rust 系统打开 port，并始终拒绝 WebView 窗口创建；邮件文档不能导航顶层窗口或获得前端通用 opener 权限。
- React 不参与链接点击，不订阅 URL 事件，也没有接受任意 URL 的 Command。危险 scheme、本机路径和混淆目标在清洗与宿主回调两处均被拒绝。

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
3. **系统外链与布局修正**：安全链接由宿主复验后直接系统打开，恢复传统表格邮件的 class/ID、宽度、居中、间距和对齐语义，并补恶意 scheme 与迁移回归。
4. **完整回复/转发**：高保真内容导入、编辑器 schema、签名布局、三格式 round-trip 和附件/线程回归。

每个批次单独完成自动验证并交付手动验收；用户确认前不进入下一批。

## 第一批实施结果（已验收）

- 新增 `testdata/mail-rendering/` 正式共享语料和 manifest，覆盖无样式、交易表格、响应式营销、原生深色、混合背景、普通链接/远程资源与主动恶意内容；全部为无真实身份信息的合成邮件。
- Rust 清洗回归对完整语料固定严格 CSP、脚本/事件/表单/嵌入文档、危险 scheme、CSS 网络资源和固定遮罩边界；当时前端按第一批基线精确验证 `sandbox=""`、无 `allow` 与 `no-referrer`，第三批最终方案已按修订后的 ADR 0008 将唯一 token 改为 `allow-popups`。
- 第一批提出的 ADR 0008 原方案为不透明 ID、自定义协议窄桥接、离站确认和 Rust 系统打开；其 Windows 实机结果在第三批推翻了桥接部分，现以本 iteration 的第三批修正记录和修订后 ADR 为准。
- 没有改变生产 HTML 清洗、主题、远程图片、外链、Capability、IPC 或回复/转发行为；第二至四批范围保持不变。
- `cssparser 0.37` 已由现有依赖间接进入锁文件，但其 MPL-2.0 许可证不属于项目默认宽松许可证集合；用户在本批验收时已明确确认第二批可以把它声明为直接依赖。

自动验证：

- `pnpm test`：21 个测试文件、45 项测试通过。
- `pnpm build` 通过，保留主入口大于 500 kB 的既有提示。
- `cargo fmt --all -- --check` 通过。
- `cargo test --offline --locked`：68 项测试通过。
- `cargo clippy --offline --locked --all-targets -- -D warnings` 通过。
- `git diff --check` 通过；未运行 Tauri bundle。

手动验收：

- 2026-07-21：用户在 Windows 确认现有阅读器无回归，接受 ADR 0008，并明确确认第二批可以直接使用 MPL-2.0 的 `cssparser 0.37`。第一批验收通过。

## 第二批实施结果（已验收）

- 新增独立 Rust CSS 安全模块；`<style>` 与行内 `style` 都由 `cssparser 0.37` 解析、按展示属性/选择器/媒体查询白名单重建，并在 CSS 解码后阻止 raw-text 逃逸。
- 安全保留常见邮件排版、表格、颜色、字体、背景渐变、普通/属性选择器，以及受控响应式和 `prefers-color-scheme`；外部样式、CSS 网络 URL、未知函数、非 `@media` at-rule、固定遮罩、动画和变换继续移除。
- CSS 设置 256 KiB 样式表/总输出、16 KiB 行内样式、2 KiB 选择器、4 KiB 声明值、2,048 条规则和 8 层值嵌套预算；超限内容按安全失败方式丢弃。
- 阅读器浅/深色默认样式移除 `!important`，有效 `color-scheme` 同步设置到 iframe 与内部文档。无样式邮件仍使用 NextMail 配色，作者明确的页面、类和行内颜色按正常层叠优先；完整 HTML 的 `<body style>` 转换为安全的内部正文容器，sandbox、CSP、远程图片和 `no-referrer` 不变。
- 新增迁移 0010，将 SQLite 数据格式提升到 10，并失效旧 `safe_html` 缓存；纯文本正文不受影响。正文请求优先从账户隔离的本地原始 EML 在 blocking worker 中重新解析/清洗并事务写回，只有本地原文缺失或不可解析时才访问 IMAP。外链和完整回复/转发没有提前进入本批。

自动验证：

- `pnpm test`：21 个测试文件、46 项测试通过。
- `pnpm build` 通过，保留主入口大于 500 kB 的既有提示。
- `cargo fmt --all -- --check` 通过。
- `cargo test --offline --locked`：79 项测试通过。
- `cargo clippy --offline --locked --all-targets -- -D warnings` 通过。
- `git diff --check` 通过；未运行 Tauri bundle。

手动验收：

- 2026-07-21：用户在 Windows 确认第二批功能正常，阅读器 HTML/CSS 保真、深色模式及既有安全边界验收通过。

## 第三批实施结果（已验收）

- 第一版不透明 link ID、自定义协议和离站确认完成自动验证后，在 Windows WebView2 实机出现“点击无任何反应”，未通过手动验收。用户明确要求移除确认与链接隔离，改为系统默认程序直接打开；该原型对应的事件、Command、React 状态和存储代码已撤销。
- Rust URL 边界仍只接受规范化后的 `http`、`https`、`mailto`；拒绝相对地址、本机文件、其他 scheme、用户信息、反斜线、控制字符、双向文本控制符和百分号编码控制字符。安全目标直接保留在 `href`，链接固定为 `_blank` 和 `noopener noreferrer`。
- iframe sandbox 仅增加 `allow-popups` 以让真实点击到达宿主。主窗口改由既有平台配置显式创建；`on_new_window` 用同一 Rust 边界复验目标、调用 `state.rs` 注入的系统打开器，并始终 `Deny` 应用内窗口。没有开放 scripts、forms、same-origin、top-navigation、前端 IPC 或任意协议。
- 实机对比确认布局异常来自清洗与阅读器覆盖：此前删除 `class`/`id` 及表格传统属性，使固定宽度居中容器和作者 CSS 失效；全局字体、16px 内边距、任意断词和 `img/table max-width:100%` 又进一步改变几何布局。最终实现保留 class/ID、表格宽高/间距/对齐/背景色、`nowrap` 和传统字体属性，并移除上述全局覆盖。
- 迁移 0011 可能已在第一次实机运行应用，因此根据 SQLx 校验规则保持原文件不变；新增 0012 删除临时 `message_links` 表、把数据格式提升到 12，并再次失效旧 `safe_html`，由本地原始 EML 按最终规则重建。没有新增依赖、前端 Capability 或第四批回复/转发功能。

自动验证：

- `pnpm test`：21 个测试文件、46 项测试通过。
- `pnpm build` 通过，保留主入口大于 500 kB 的既有提示。
- `cargo fmt --all -- --check` 通过。
- `cargo test --offline --locked`：85 项测试通过，覆盖安全 `href`、危险 URL、系统打开前复验、传统表格布局/class 选择器，以及 0011→0012 修正迁移。
- `cargo clippy --offline --locked --all-targets -- -D warnings` 通过。
- `git diff --check` 通过；未运行 Tauri bundle。

手动验收：

- 2026-07-21：用户在 Windows 确认修正后功能正常。HTTPS/`mailto:` 系统打开、无确认框/应用内外部网页窗口，以及固定宽度营销邮件的整体居中、横幅、内容卡片和文字换行均验收通过。
- macOS 未执行，不记录通过。

## 非目标

- 不允许邮件脚本、事件处理器、表单提交、嵌入页面、same-origin、顶层导航、任意网络或任意文件权限。
- 不实现远程内容代理、CID/内联附件完整协议之外的通用浏览器能力；若这些成为保真前提，需要单独扩展计划。
- 不实现搜索、会话、托盘、通知、POP3 或 OAuth。
- 不借本阶段进行全局性能、工具链、CI、发布或无关 UI 重构。

## 自动验收

- Rust：HTML/CSS/URL 恶意语料、样式保留、远程资源阻断、缓存失效和账户隔离。
- 前端：sandbox/CSP、浅色/深色默认、链接新窗口边界、编辑器导入/编辑/保存、回复/转发布局和签名节点。
- 完整运行 `pnpm test`、`pnpm build`、Rust fmt/test/Clippy 和 `git diff --check`，不执行 Tauri bundle。

## 手动验收

- Windows 真实邮件覆盖无样式正文、复杂表格、营销邮件、浅/深色邮件、远程图片和多种外链。
- 安全链接不显示 NextMail 确认，直接由系统浏览器/邮件程序处理；NextMail 内不得加载外部网页，危险或本机目标不得产生动作。
- 回复、回复全部和转发重开草稿后仍保留完整可编辑原文、顶部空白回复区、默认签名、线程头和附件。
- macOS 只有在真实设备执行后才记录通过。
