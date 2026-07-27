# ADR 0011：已收邮件本地内联图片边界

状态：已接受

日期：2026-07-22

## 背景

HTML 邮件可以用 `cid:` URL 引用同一 MIME 消息中的其他 body part。RFC 2392 定义了 `cid:` 与 `Content-ID` 的对应关系，RFC 2387 定义了 `multipart/related` 复合对象。部分发件客户端同时把被引用图片标为 `attachment`；若接收端只相信 `Content-Disposition`，会把正文图片重复显示成普通附件。

NextMail 的阅读器此前会移除 `cid:`，并把 `mail-parser` 返回的全部 attachment part 列在附件区，导致正文缺图和附件误分类。直接开放 iframe 访问本地文件、自定义协议或任意网络会扩大 ADR 0002 的权限边界。

## 决策

- Rust 从当前账户隔离、已经下载的原始 MIME 中解析 HTML 与附件 part；不由 React 解析 MIME，也不把路径或内容哈希交给前端。
- 只有 HTML 包含对应 `cid:`、part 有可规范化 `Content-ID`、类型为 PNG/JPEG/GIF/WebP、单项不超过 25 MB 且单封累计不超过 100 MB 时，才把解码字节转换为内存 `data:` URL。`cid:` 按 RFC 2392 对 URL 部分执行一次 `%hh` 解码，再与完整 MIME part 集合的 Content-ID 索引匹配，不依赖解析器对 `Content-Disposition` 的附件/inline 分类。
- 邮件 HTML 自带的 `data:image` 只有在 media type、base64/百分号编码、解码后文件魔数和共享大小预算全部通过时保留；允许 PNG/JPEG/GIF/WebP，拒绝 SVG、HTML、未知类型、错误编码和超限内容。
- 转换后的 URL 在既有 Rust HTML 白名单清洗期间替换 `img src`，最终文档 CSP 仍只有 `img-src data:`；不新增脚本、同源、表单、顶层导航、任意文件或网络权限。
- 只有真正进入清洗结果的 CID part 才从普通附件摘要排除。声明为 PNG/JPEG/GIF/WebP 的 part 沿用受限内联规则；被错误声明为 `application/octet-stream` 的 CID part 只有在解码字节文件魔数可确认上述四种图片类型时才兼容内联，不信任文件名。未引用、超限、类型不受支持、文件魔数不匹配或无法安全解析的 part 继续作为附件，不静默丢失；从本地 EML 重建正文时，正文和重复附件投影在同一 SQLite 事务更新。
- 数据库仍以原始 EML 为重建来源，不把展开后的图片另存进 `safe_html` 之外的新资源目录；Composer 的受管 CID 附件与发件边界继续由 ADR 0009 管理。

## 影响

- Foxmail 等客户端生成的正文引用图片可离线显示，即使其 `Content-Disposition` 为 `attachment`，也不会在附件区重复出现。
- 大型或非图片 CID part 不内联，避免无界 data URL 与主动内容类型进入阅读器。
- 清洗后的 `safe_html` 会包含受限 data URL，数据库空间可能比纯 HTML 增加；原始 EML 仍可用于未来策略迁移和重建。
- 远程 `http(s)` 图片、CSS 背景资源和 Web Font 不受此决策影响，仍遵循默认阻止策略。

## 验证门禁

- MIME 回归覆盖 `Content-Disposition: inline`、`attachment` 和缺失 disposition，以及 `%hh` CID、大小写差异、未引用 Content-ID、阿里云式 `application/octet-stream` 图片、伪装图片和嵌套 multipart，并断言正文 data URL 与附件投影正确。
- HTML 回归覆盖合法 raster data URL、错误文件魔数、SVG、错误编码和大小预算。
- 既有主动内容、CSP、sandbox、远程资源和账户隔离回归必须继续通过。
- Windows WebView2 与 macOS WKWebView 的最终显示分别实机验收；未经执行不宣称对应平台通过。
