# 邮件深色模式适配重构

状态：已验收

## 实施结果

- Rust 在清洗前识别四类作者深色信号，安全文档只携带 `data-nextmail-native-dark` 内部标记；`data-ogsc` / `data-ogsb` 作为惰性样式钩子保留，脚本、事件和危险资源边界不变。
- 新增纯前端静态 cascade：使用惰性 `DOMParser`、隔离 CSSOM、原生 `querySelectorAll`、specificity、源码顺序、`!important`、inline style、继承、`bgcolor`、`font[color]`、`hr[color]` 与表格 `bordercolor` 计算清洗后颜色；Rust 清洗器同步保留这些安全表现属性，避免浅色模式回落到 UA 默认颜色。
- Smart Inversion 将 HSL Lightness 映射到 9%–90% 深色范围，保留 Hue、Saturation 与 Alpha；高饱和背景先按感知亮度压到暗色表面上限。文字对比度不足时优先继续压暗其有效带色背景以保留红色等作者语义色，仍不达标才最小调整文字明度，最终执行 4.5:1 WCAG 对比度约束。
- 最终颜色以 `!important` inline style 写回字符串；原生深色邮件跳过该过程，阅读 iframe 继续只允许 `allow-popups`，不增加 same-origin 或 scripts。
- 图片与视频保持清洗后的原样，不增加透明图片衬底；作者边框相对有效背景校正到至少 3:1，未声明颜色的既有边框使用统一可见兜底。HTML 邮件恢复外围内边距，默认白色邮件背景映射为与 App 阅读面板相同的 `#171717`，避免形成异色正文框；App 深色表面令牌统一为 RGB 等值的零色度灰阶。无法解析的 CSS Color 4 值、渐变内部颜色及清洗器本就拒绝的复杂选择器不做推测性转换。

## 目标

以统一的 Smart Inversion 替换按文字、区块、表格等元素类型不断补丁的邮件深色适配，在保持品牌色相与图片内容的前提下，让缺少原生深色样式的 HTML 邮件获得稳定的深色阅读效果。

## 范围

- 识别邮件声明的 `color-scheme`、`supported-color-schemes`、`prefers-color-scheme: dark` 和 Outlook `data-ogsc` / `data-ogsb` 深色适配信号。
- 作者已提供深色适配时保留并信任其安全清洗后的深色样式，避免二次转换。
- 作者未提供适配时统一转换前景、背景和边框颜色，并校验文字与背景的 WCAG 对比度。
- 图片与视频不做内容级颜色分析或样式改写，保持原样。
- 继续保留远程图片默认阻止、Rust 权威清洗、无脚本 iframe 与受控外链边界。

## 已确认约束

- 当前阅读 iframe 使用 `sandbox="allow-popups"`，明确没有 `allow-same-origin`；父 WebView 不能读取 iframe DOM 或调用其 `getComputedStyle()`。
- 邮件不得进入主 React DOM，也不得为视觉适配启用脚本。
- 当前 Rust 清洗保留受控 `prefers-color-scheme` 媒体规则，但移除 `<meta>` 与 `data-ogsc` / `data-ogsb`；若要识别这些信号，应在清洗前检测并只向清洗后文档传递布尔标记。
- 放宽 `same-origin` 属于安全边界变化，必须得到用户明确决策、补针对性测试并新增 ADR；不得作为视觉实现的隐含副作用。

## 实现路径

- Rust 在权威清洗前检测作者深色适配信号，向清洗后的安全文档传递单一内部标记。
- 前端只解析已经清洗的 HTML；使用惰性 `DOMParser` 文档、浏览器 CSSOM 和原生选择器匹配完成静态 cascade，不把邮件节点挂入主 DOM。
- cascade 只处理清洗器允许保留的颜色属性、选择器与受控媒体查询；计算 specificity、源码顺序、`!important`、inline style、继承及常见 `bgcolor`、`font[color]`、`hr[color]`、表格 `bordercolor` 表现属性。
- 对最终颜色执行 HSL 明度转换，保留 Hue、Saturation 和 Alpha；带色背景增加暗色表面感知亮度上限，文字不足 4.5:1 时先压暗其背景、再按需微调文字，最后以 `!important` inline style 写回。
- 转换后的字符串继续进入原有无脚本、无 same-origin 的 opaque sandbox iframe。
- 不引入 CSS cascade 第三方依赖；复杂 CSS 已在 Rust 清洗阶段被拒绝，无法解析的个别颜色或规则保持原值并由阅读区默认深色表面兜底。

## 非目标

- 不分析或重绘图片内容。
- 不放开脚本、表单、任意网络、文件、top-navigation 或邮件内 Web Font。
- 不改变 Composer 原文与编辑器渲染。

## 验证门禁

- Rust 覆盖深色适配信号检测、标记传递、清洗后危险内容不回流。
- 前端覆盖主题切换、原生深色跳过、自动适配、远程图片 CSP 与 iframe sandbox。
- 使用 `testdata/mail-rendering/` 覆盖无样式、复杂表格、营销邮件、原生深色、高亮底色、透明图片和恶意内容。
- 运行相关 Vitest、`cargo fmt --check`、`cargo test --offline --locked`、`cargo clippy --offline --locked --all-targets -- -D warnings`、`pnpm build` 与 `git diff --check`。

## 验证结果

- 2026-08-11：Windows 实机验收通过。
- `pnpm build` 通过；仅保留仓库已知的大 chunk 警告。
- `cargo fmt --all -- --check` 通过。
- `cargo test --offline --locked` 通过，共 169 项测试。
- `cargo clippy --offline --locked --all-targets -- -D warnings` 通过。
- `git diff --check` 通过。
- 完整 Vitest 因沙箱外执行审批超时未运行；前端 TypeScript 与生产构建门禁已通过。
- 本阶段变更随 `v0.4.0` 发布。
