# NextMail 项目开发手册

更新时间：2026-08-11

本文是 NextMail 新会话唯一必读的长期技术文档，集中保存当前实现事实、开发细则、工程约定和项目记忆。历史范围与验收记录保留在 `iterations/`；重大架构或安全取舍保留在 `adr/`，都只在任务相关时按需查阅。

代码、配置、迁移和测试是最终事实来源。若本文与当前检出内容不一致，先核对实现，再在同一批修改中修正文档。本文不记录某次会话的分支、HEAD、未提交文件或复制粘贴式交接快照。

## 1. 新会话开始方式

接手项目后：

1. 完整阅读本文。
2. 执行 `git status --short` 和 `git log -3 --oneline --decorate`，确认当前 HEAD、远端关系和用户未提交修改。
3. 阅读本次任务涉及的源码、配置、迁移和测试；需要历史范围或设计理由时，再查相应 iteration/ADR。
4. 只实施用户明确给出的当前计划，不从“后续设想”中自行选择功能。
5. 每次新的开发计划先在 `docs/iterations/` 建立或更新按 `YYYY-MM-DD-NN-主题.md` 命名的计划文档，`NN` 从 `01` 起表示当天实施顺序；写清状态、范围、非目标和验证门禁，不再分配全局阶段编号。实施结果、验证与验收继续写回同一文件。
6. 不得 reset、覆盖、清理或顺手提交用户已有修改；出现重叠时先说明。
7. 按风险完成验证，交付结果和必要的实机验收步骤。

## 2. 产品与当前状态

NextMail 是基于 Tauri 2、React/TypeScript 和 Rust 的本地优先桌面邮件客户端，当前版本为 `0.4.0`。

平台边界：

- Windows 10 22H2+ x64 是主要开发与实机验收平台。
- macOS 12+ Intel/Apple Silicon 是正式目标平台；未实际执行的行为不得宣称通过。
- Linux x64 提供实验性 bundle，不做深度适配或完整实机承诺。
- 当前产物没有 Windows 正式代码签名，也没有 Apple Developer 签名或公证。

已经实现：

- 首次启动、可迁移数据目录、账户自动发现和真实 IMAP/SMTP 密码认证。
- 多账户添加、编辑、重新认证、切换、安全移除、独立同步策略和文件夹角色映射。
- SQLite 离线视图、全服务器头部优先同步、按需正文、账户可选全文同步和逐封可见更新。
- 已读、星标、移动、复制、归档、删除的本地乐观更新与持久化 IMAP 待办。
- 在线 IMAP 文件夹创建、重命名、层级移动、删除、全部已读及独立本地排序。
- RFC 2047、RFC 2231 连续附件名（包括先分段后形成完整 encoded-word 的兼容形态）、常见 MIME 字符集、IMAP modified UTF-7。
- Rust 清洗的 HTML/CSS 阅读、远程图片控制、标准 CID、受限 data 图片、误标 octet-stream 图片和 BMP。
- 原始 EML、附件按需下载、安全另存为、受控系统打开和所在文件夹定位。
- 当前账户/当前文件夹 SQLite FTS5 搜索；Enter 或搜索按钮显式提交。
- Tiptap/ProseMirror + CodeMirror 富文本写信、源码/预览、带 To/Cc/Bcc 覆盖的模板、签名、变量、回复/转发和内嵌图片。
- 关闭 Composer 时显式决定是否保存；持久化 SMTP 发件、Sent/Drafts APPEND 和 Drafts 定向刷新。
- Windows/macOS 窗口壳、窗口状态记忆、独立设置/账户/定义/预览/原文窗口。
- 中英文、系统/浅色/深色主题、主题色和分层通知偏好。
- 账户隔离的本地联系人、既有邮件后台回填、收信自动沉淀、联系人工作区、最近往来、姓名优先、身份名片/复制/编辑/删除和 Composer 联系人建议。
- 邮件与联系人列表支持 Ctrl/Cmd、Shift 范围多选及针对当前选择的右键操作。
- NextMail 自有通知窗口、跨平台托盘、可持久化关闭偏好，以及 `v*` tag 触发的三平台 GitHub Release 工作流。
- 基于 Tauri Updater 的签名更新检查与安装；中国大陆优先 NextMail GitHub 反代，其他地区及定位失败优先 GitHub 直连，两种传输地址互为备用。
- 可用更新使用单例独立窗口呈现；Release notes 作为不可信 Markdown 受限渲染，原始 HTML、图片和危险协议不会进入页面。

当前实施状态：

- `0.4.0` 已完成并通过 Windows 实机验收：统一重构桌面表面层级、标题栏边界、信息密度、中性灰阶深色基调及键盘焦点态；邮件深色模式在不放宽 opaque sandbox 的前提下，增加作者原生深色识别、安全 CSS cascade、HSL 明度转换和文字/背景/边框对比度校正。
- `0.3.1` 已完成 Composer 地址栏、联系人候选键盘操作、富文本 Tab、邮件标题与按钮 hover 优化，并为回复/转发提供同一行的明确原文标题；本轮已通过自动验证和用户手动验收。
- `0.3.0` 已完成独立安全更新窗口、全局对话框层级修正、设置选择项交互优化、分段 MIME 附件名兼容与新应用图标；四平台 Release workflow、自行生成的直连/大陆代理 updater 清单及公开发布链已经实际运行并验收通过。

当前计划见 [`2026-08-11-03-mail-dark-mode`](./iterations/2026-08-11-03-mail-dark-mode.md)；最近完成记录见 [`2026-08-11-02-ui-reconstruction`](./iterations/2026-08-11-02-ui-reconstruction.md)。

仍未排期：

- IMAP IDLE、无 IDLE 轮询和秒级失败重试。
- 会话聚合、跨账户搜索、统一收件箱。
- 跨机账户重绑定。
- 系统通知中心历史/勿扰集成、正式代码签名与公证。
- 规则、日历、PGP/S-MIME、EML/MBOX 导入导出和 Linux 深度适配。

这些只是长期记忆，不是自动生效的路线图。下一次开发以用户当前计划为准。

## 3. 技术栈与仓库地图

前端使用 React 19、TypeScript 5.8、Vite 7、TanStack Query 5、react-i18next、react-markdown 10、Tailwind CSS 4、Radix Primitives、Tiptap/ProseMirror 3.27.3、CodeMirror 6、Vitest、Testing Library 和 jsdom。

桌面/Rust 使用 Tauri 2、Tauri Updater、Tokio、async-imap 0.11、lettre 0.11、mail-parser 0.11、mail-builder 0.4、SQLx 0.9、SQLite WAL/FTS5、rustls 0.23、keyring 4.1、Ammonia 4 和 cssparser 0.37。精确版本以 lockfile 和 manifest 为准。

```text
src/
  app/                  IPC、DTO、Query key、外观、语言、平台入口
  components/ui/        自有基础组件与滚动容器
  components/window/    跨平台标题栏
  features/             accounts/composer/contacts/mail/notifications/onboarding/preferences
  locales/              zh-CN、en-US
  styles/               语义主题与全局样式
src-tauri/
  capabilities/         各窗口最小权限
  migrations/           只增不改的 SQLx 迁移，当前到 0026
  src/core/             无 Tauri/SQLx/协议库依赖的 DTO、错误与 ports
  src/application/      账户生命周期与纯业务组合用例
  src/adapters/         JSON、Keyring、发现、连接测试和系统集成
  src/protocols/        IMAP/SMTP/MIME/HTML/TLS Adapter
  src/storage/          SQLite 与内容存储的窄 Repository
  src/commands/         薄 Tauri Command
  src/state.rs          唯一组合根
  src/mail_runtime.rs   账户 Supervisor 门面
  src/composer_runtime.rs
  src/notification_runtime.rs
app-icon.png                README 展示与 Tauri 各平台图标的唯一源图
testdata/mail-rendering/ 正式邮件保真与恶意内容回归语料
docs/project.md          本文
docs/iterations/         每次迭代范围、变更摘要、验证与验收
docs/adr/                按需查阅的长期架构决策
```

仓库只有 `src-tauri/Cargo.toml` 一个 Rust package。根目录不得出现 Cargo Workspace、根 `Cargo.toml`、根 `Cargo.lock`、根 `target` 或业务子 crate。

## 4. 不可破坏的架构边界

### 前端与 IPC

- React 不直接访问 SQLite、邮件服务器、任意文件系统或系统凭据库。
- 业务组件统一通过 `src/app/api.ts` 和稳定 DTO 调用 Command，不散落裸 `invoke`。
- `src/app/types.ts` 与 Rust DTO 维持 camelCase 序列化契约。
- TanStack Query 管理读取模型；Event 只携带公开 ID、状态、进度或修订并触发精确失效/重读，不推正文。
- Query key 使用集中工厂；邮件详情稳定为 `['message', accountId, mailboxId, messageId]`。
- 外观偏好由各 WebView 的 TanStack Query 管理，窗口间通过 Rust 持久化值和窄事件同步。

### Rust 分层

- `core` 不依赖 Tauri、SQLx、`tracing` 或具体协议库。
- `application` 组织账户生命周期、回复/转发、定义变量等纯业务用例。
- `adapters`/`protocols` 隔离系统和第三方类型；`storage` 提供窄 Repository。
- `state.rs` 是组合根，具体配置、凭据、协议、Repository、系统 opener 和通知通过 ports 注入。
- Command 保持薄，只做 DTO 接收、用例委托、稳定错误转换和窄事件发布。
- 公开失败统一为 `CommandError { code, params, retryable }`；UI 不得得到密码、Token、服务器原始响应、内部路径或堆栈。

### 窗口与 Capability

窗口包括 `main`、`composer-*`、`settings`、`accounts`、`definition-*`、`message-preview-*`、`raw-message`、`notification-*`、`update`。

- 每类使用独立最小 Capability；前端没有 Shell、数据库、任意网络、任意文件或任意建窗权限。
- `settings`、`accounts`、`raw-message`、`update` 是单例；动态窗口按稳定业务目标复用。
- 普通窗口先隐藏，React 懒加载和首批 Query 完成后显示；错误边界也要显示。
- 窗口状态由 Rust 侧 `window-state` 保存；动态标签映射为公共类别，通知窗口不保存。
- Windows 使用 React 自绘控制；macOS 使用 Overlay 和原生交通灯。
- 业务窗口标题随语言变化；主窗口和通知保留 `NextMail`。
- 系统托盘由 Rust 创建并按界面语言更新菜单；Windows/macOS 左键显示主窗口，右键提供显示、设置、退出。Linux 受 Tauri/AppIndicator 限制不产生托盘点击事件，左键显示菜单并通过“显示主界面”恢复窗口。
- 主窗口关闭请求由 Rust 统一拦截。默认询问最小化到托盘或退出；关闭询问后由设备级偏好直接隐藏或退出。托盘不可用时不得隐藏成无法恢复的窗口，托盘菜单“退出”作为明确动作不二次询问。

## 5. 数据、同步与协议记忆

### 数据与配置

```text
.nextmail-data.json
content.sqlite
raw/<hash-prefix>/<hash>
attachments/<hash-prefix>/<hash>
cache/attachment-open/...
```

设备级托盘、关闭与更新偏好保存在系统应用配置区的 `config/desktop-preferences.json`，不随邮件数据目录迁移。

- SQLite schema metadata 当前为版本 26，迁移到 `0026`；migration 编号是本地数据格式序号，不等于产品阶段编号。
- `.nextmail-data.json` 的 `format_version` 当前为独立版本 1，不是 SQLite schema 版本。
- 已发布迁移只允许新增，不得修改。
- 所有账户业务数据按匿名 `account_slot_id` 隔离。
- 多表可见状态使用 SQLx 事务；网络、MIME 和慢文件 I/O 不持有 SQLite 写锁。
- 内部路径和内容哈希不返回 React。
- 账户配置、本机偏好和窗口状态在系统应用配置区；密码及未来 Token 只进入服务名 `com.taurusxin.nextmail` 的系统凭据库。

### 联系人投影

- `contacts`、`message_contacts`、`contact_backfill_state` 全部按匿名 `account_slot_id` 隔离；邮箱规范化只清理首尾空白并折叠 ASCII 大小写，不做服务商别名合并。
- IMAP 解析收集 `From`、`Sender`、`Reply-To`、`To`、`Cc`、`Bcc`，与邮件投影在同一短事务内幂等写入；首次自动创建优先采用有效的邮件头显示名，没有有效显示名时才取邮箱前缀。邮箱一旦匹配本地联系人，后续邮件头不得改写其姓名。
- 既有邮件后台回填使用稳定 message rowid 游标，每批 200 封，只能从已持久化的 `From`、`To`、`Cc` 恢复；回填状态可中断续跑，变化事件按批次合并。
- 联系人邮箱是不可变身份键；允许新增、改名和账户内直接删除。删除会级联移除本地邮件关联，后续同步再次发现同邮箱时允许按现有自动命名规则重新创建。
- 邮件持久化头部名称保持原样；读取时批量生成 `AddressPresentation`，展示优先级为当前账户联系人姓名、邮件头/草稿名称、邮箱。禁止 React 逐行查询或跨账户借用身份。
- 联系人事件为 `contacts-changed { accountId, revision }`，只触发账户范围的联系人、邮件列表、详情和 Composer 查询失效，不携带联系人或邮件内容。

### 同步模型

完整账户同步只有四类入口：首次设定账户、应用启动、账户配置的 1/5/10 分钟周期、用户手动收取；`0` 表示仅手动。设置变化只重置计时。持久化待办、Sent/Drafts APPEND 和 Drafts 定向刷新不触发完整同步。

同步遍历全部可选文件夹，以最多 100 UID 为网络批次，但每封邮件头单独原子落库并发布最小事件。当前文件夹按事件顺序重读本地视图，100ms 只合并绘制，不等待整批完成。

默认只同步邮件头。打开缺失正文时优先从本地原始 EML 重建，否则按需联网；加载期间正文区域只显示 spinner，失败后再显示错误与重试。启用“收取邮件全文”后，每个文件夹头部完成再补正文；正文回填不产生新邮件候选，Drafts 定向刷新跳过全文阶段。

所有账户共享两个高层网络许可。每账户最多三条按需建立、操作后关闭的主动 IMAP 会话；完整同步只占两条，为正文、附件、文件夹结构和待办保留第三条。这是有界容量，不是长期 Session 池。

已读、星标、移动、复制、归档和删除支持单封或同账户同文件夹批量 ID，在同一事务更新本地投影并写入 `pending_operations`。Worker 有序重放；异常退出可恢复。缺少 UIDPLUS 时不得执行宽泛 EXPUNGE。

文件夹结构变更和全部已读必须在线成功后再事务更新本地投影。结构操作使用账户级 mailbox 路径写锁，普通同步/正文/待办/APPEND 使用读锁；锁不跨 SQLite。文件夹本地排序不改变服务器路径。

### 草稿与发件

- 草稿保存 Tiptap JSON、HTML、纯文本和 revision；停止输入不会自动保存。
- 关闭 Composer 只有取消、不保存、保存为草稿三个显式动作。
- 保存后排入 Drafts APPEND；确认后只定向刷新 Drafts，失败由后续同步修复。
- 回复/转发保持回复区、签名分隔、签名、空行、原始邮件元数据和完整原文的稳定边界。
- 原文保存在 `nextmailOriginalMessage` 原子节点的 `sourceHtml`，不进入 ProseMirror 邮件表格 schema。
- 模板/签名支持全局和账户范围；模板可保存 To/Cc/Bcc、邮件标题和正文，所有邮件内容字段均可留空。手动选择或四场景规则自动套用时逐字段只覆盖模板中的非空值，空字段保留草稿原值；账户模板复用自身账户联系人建议，全局模板编辑时只使用最近选中的单一账户提供候选，不合并跨账户联系人。每个范围一个默认签名和自动插入开关。
- Composer 图片进入账户隔离的内容寻址存储并以 CID 发件；远程图片不静默下载。
- SMTP 前生成不可变 MIME/Message-ID，写入 `raw/` 后创建持久化 `send_job`；重试复用相同 MIME。
- SendWorker 账户内 FIFO、账户间轮转；全局最多两封、每账户最多一封。SMTP 成功后独立 APPEND Sent，归档失败不得再次发信。
- 客户端头为 `X-Mailer: NextMail/0.4.0`；该值由 Cargo package version 自动生成，版本变化时同步核对各 manifest。

## 6. 安全边界

- TLS 严格验证系统信任链和主机名，不提供忽略证书错误选项。
- 明文 IMAP/SMTP 必须由用户明确确认，并在 Rust 边界复验。
- SMTP 测试不得发送邮件；IMAP 测试不得修改邮箱。
- HTML 邮件由 Rust 权威清洗，并在无 scripts、forms、same-origin、top-navigation、任意文件或任意网络能力的 sandbox iframe 中渲染。
- 深色适配不得改变上述隔离：Rust 只从原始 HTML 提取“作者支持深色”布尔信号并写入内部标记；前端只对清洗后的字符串使用惰性 DOM、CSSOM 与原生选择器匹配，邮件元素不挂入主 DOM，最终仍作为字符串交给 opaque iframe。
- 阅读 iframe 仅保留受宿主拦截的 `allow-popups`；外链经 Rust 再次校验后交给系统，WebView 始终拒绝创建外部页面。
- Tauri 发布构建只禁止对 `style-src` 自动追加资产 nonce/hash，使 `srcdoc` 继承的父 CSP 不会阻止经过 Rust 清洗的邮件 `<style>` 与行内样式；`script-src` 等其他指令仍保留 Tauri 的资产 CSP 加固，邮件 sandbox 和子文档 CSP 不变。
- 远程图片默认由邮件 CSP 阻止；显式允许后仍使用 `no-referrer`。
- CSS 网络 `url()`、`@import`、`@font-face`、固定遮罩、动画和变换继续移除。
- CID/`data:image` 只接受经过 media type、解码、文件魔数和大小预算验证的 PNG/JPEG/GIF/WebP/BMP。
- 只有正文实际引用且成功内联的 MIME part 才从附件列表排除。
- 富 HTML 粘贴必须先经 Rust 清洗和选择器作用域限定。
- 日志不得记录 `CommandError.params`、凭据、Token、邮件正文或服务器原始响应。
- Updater 只接受内置公开密钥验证成功的签名产物，不提供无签名降级。Geo 结果只用于本次清单路由，不持久化或写日志；反代只能改变传输地址，不能改变更新信任根。完整决策见 [ADR 0016](./adr/0016-signed-updates-and-regional-delivery.md)。
- Updater Release notes 是不可信输入：Rust 限制其长度，独立更新窗口仅解析受限 Markdown，不允许原始 HTML、图片或危险链接协议；HTTP(S) 外链仍须通过 Rust 受控 opener 复验。

任何邮件内容、协议、凭据、Capability、外链、文件或网络权限放宽都属于安全变更，必须补针对性测试；重大取舍新增或修订 ADR。

## 7. 前端与体验约定

- 业务页面优先组合 `src/components/ui/`；不暴露浏览器默认表单外观。
- 主题使用语义令牌，不在业务组件写死主题相关颜色。
- 深色主题以 RGB 等值的零色度黑灰表面建立层级，不使用带明显蓝色偏向的底色；用户主题色只用于主操作、选中、焦点和必要状态，不得扩散为窗口背景基调。
- 新生产文案同时提供 `zh-CN` 与 `en-US`，缺失时回退英文。
- UI 不显示调试说明、内部阶段名或临时占位文案。
- Windows 使用 Segoe UI/Microsoft YaHei UI，macOS 使用系统 UI/PingFang SC。
- 根目录 `app-icon.png` 是 README 与 Tauri 应用图标的唯一源图；更新后使用 `pnpm tauri icon app-icon.png` 统一刷新 `src-tauri/icons/`，不手工维护不同平台尺寸。
- 自绘标题栏使用独立语义表面和永久下边界，窗口标题居中显示，不在左上角重复；Windows 保留自绘窗口控制，macOS 继续使用原生交通灯。窗口失焦时只降低标题与自绘控件强调度，仍保留标题栏边界。
- 桌面层级统一使用主题中的表面、边界、高光与阴影令牌；可调整窗格的分隔边界常驻可见，hover 与键盘 focus 只增强反馈，不承担首次发现职责。
- 用户选择的主题色作为持久化源色保留；应用到界面的主色按当前亮色/深色表面自动校正明度，并同步选择黑色或白色前景，使主色文字与主操作保持至少 4.5:1 对比度。跟随系统主题时应监听系统明暗变化并重新计算，不改写用户保存的源色。
- 通用标题和正文允许超长连续内容换行；固定窗口最小尺寸使用逻辑像素，主窗口在 920px 最小宽度下必须保留侧栏、列表和至少 372px 阅读区。操作按钮组允许换行，不得用溢出或截断隐藏关键操作。
- 所有模态与阻塞进度遮罩统一使用全局对话框层级，覆盖完整 WebView、位于 Windows 自绘标题栏之上并显式退出拖动命中区域；设置页布尔项以复选框、标题和说明组成内容包裹式圆角点击区域，提供明确 hover、focus、选中与禁用反馈。
- 可点击控件统一使用桌面默认指针，不显示手型 pointer；文本选择、窗格缩放和拖动等非点击交互继续保留 I-beam、resize、grab 等语义指针。
- 不移除键盘焦点指示。普通操作与列表行仅在 `focus-visible` 时使用 1px 内描边，输入和选择等编辑表面使用克制的 2px 内反馈；鼠标点击不应产生截图中可见的粗外框。
- 紧凑横向工具栏只直接展示高频操作，低频操作收入有明确名称的更多菜单；不得依赖横向滚动条隐藏工具栏操作。
- 任何可能产生纵向滚动的容器统一使用 `OverlayScrollArea`：滑块绝对覆盖在容器右侧，不参与内容宽度计算，不为滑块预留 `padding`、gutter 或空白，出现与消失不得改变内容和分割线坐标；滑块宽 6px，仅在容器 hover 或键盘 focus-within 时显示，并保持默认指针。业务组件只负责自身对称内容边距，禁止通过左右不对称边距给滚动条让位。
- 文件夹列表是滚动条位置的唯一布局例外：展开侧栏中的文件夹项不是全宽且带圆角，滚动容器向右延伸到侧栏外边距，等量 `content` 右内边距保持列表项宽度不变，使滑块位于圆角项外侧；仍不得给 viewport 预留空间或让滑块改变列表项几何尺寸。
- 保留平台差异：仅 Windows 显示 WebView 自绘窗口按钮；macOS 使用原生交通灯，其他带系统装饰的平台使用原生窗口控制。
- 邮件 HTML 与 Composer 原文不进入主 React DOM；保真优化不能越过安全边界。
- 邮件深色模式优先信任作者的 `color-scheme`、`supported-color-schemes`、`prefers-color-scheme: dark` 或 Outlook `data-ogsc` / `data-ogsb` 适配信号；其他邮件对清洗后允许保留的 `color`、`background-color` 与四侧 border color 静态计算 cascade，并把安全清洗后保留的 `bgcolor`、`font[color]`、`hr[color]` 及表格 `bordercolor` 纳入同一过程。带色背景先限制暗色表面亮度；文字对比度不足时优先继续压暗其有效带色背景以保留作者文字色，仍不达标才最小调整文字明度，最终保持至少 4.5:1；作者边框相对有效背景至少保持 3:1。默认白色邮件表面映射为与 App 阅读面板相同的 `#171717`，阅读区保留外围内边距但不形成异色框；阅读器为未声明颜色的现有边框提供统一可见兜底，图片与视频保持清洗后的原样。

## 8. 依赖、Git 与交付约定

- Node.js 只使用 pnpm。
- 当前不使用 Python；未来需要时只用 uv 管理依赖和执行环境。
- Rust 只在 `src-tauri` 单一 package 中工作。
- 新依赖优先 MIT、Apache-2.0、BSD、ISC；其他许可必须先确认并按需更新第三方说明。
- 不提前开发当前范围外行为，不自行扩展依赖、安全策略或重大 UI 决策。
- 未经明确要求，不 commit、push、创建/移动 tag、改远端、签名或发布。
- 不用 reset/checkout 覆盖工作区，不清理无关文件。
- 正式测试和测试语料长期保留；临时探针、凭据、日志、截图、coverage 和临时数据在验证后清理。
- `dist/` 和 `src-tauri/target/` 是正常增量缓存，默认保留。
- Git 历史承担逐提交细节；iteration 保留每次开发计划的范围、变更摘要、验证和验收。
- 发布新版本时保持两个顺序提交：先提交已经验收的功能、测试与实现文档，再单独提交版本号、`CHANGELOG.md` 和发布记录；不得把整轮功能变更与发布准备压进同一个 commit。
- 既有编号 iteration 作为历史保留；2026-08-09 起的新计划按 `YYYY-MM-DD-NN-主题.md` 命名，`NN` 只表示当天实施顺序，不是全局阶段编号。状态使用“规划中”“实施中”“等待手动验收”“已验收”或明确的“未排期”。
- 不重新建立会话 handoff、独立 changes 流水账或重复的 architecture/technical-reference/master-plan。

## 9. 开发、验证与发布

### 安装与运行

```powershell
pnpm install
pnpm tauri dev
```

`pnpm tauri dev` 是桌面联调入口并复用 Rust 增量缓存。纯浏览器层可用 `pnpm dev`，但不能替代 Tauri Command、Capability、Keyring、文件选择器和窗口生命周期验收。

日常不运行 Tauri bundle；仅在用户明确要求发布或打包时执行。

### 自动验证

前端：

```powershell
pnpm test
pnpm build
```

Rust：

```powershell
Push-Location src-tauri
cargo fmt --all -- --check
cargo test --offline --locked
cargo clippy --offline --locked --all-targets -- -D warnings
Pop-Location
```

最终：

```powershell
git diff --check
```

没有根 Cargo Workspace，不运行 `cargo test --workspace`。纯文档修改至少检查 Markdown 本地链接、陈旧引用和 `git diff --check`，无需运行产品构建或 Tauri bundle。

### GitHub Release

`.github/workflows/release.yml` 只响应 `v*` tag push，构建 Windows x64、Ubuntu 22.04 x64、macOS Intel x64 和 macOS Apple Silicon arm64；四组产物上传同一草稿 Release，全部成功后才公开。

每次 Release 正文从根目录 `CHANGELOG.md` 提取与当前 tag 匹配的版本段落，不使用 GitHub 自动生成的固定日志。

发布构建由 `tauri-action` 生成、签名并上传各平台 updater 产物，但不使用其自动 `latest.json`。每个构建任务把本地 `.sig` 作为 workflow artifact 交给最终发布任务；macOS 两个架构的本地签名同名，上传 workflow artifact 前必须按矩阵架构规范化为与 Release 资产一致的 `_aarch64.app.tar.gz.sig` 或 `_x64.app.tar.gz.sig`。最终任务从对应 `CHANGELOG.md` 版本段落、当前 Tag、固定平台映射和实际签名自行生成标准 `latest.json`，再派生只为每个公开下载 URL 前置 `https://proxy.next-mail.app/` 的 `latest-cn.json`。两个清单必须通过版本、11 个平台键、签名和 Tag 下载前缀验证，随后在同一步上传并公开草稿 Release，任何缺失或歧义都保持草稿并失败。客户端同时配置直连与反代清单：CN 优先反代，其他地区及 Geo 失败时优先直连，另一地址作为备用。`TAURI_SIGNING_PRIVATE_KEY` 与密码只来自 GitHub Secrets，公开验证密钥由 `NEXTMAIL_UPDATER_PUBLIC_KEY` Repository Variable 注入并固化进客户端；缺少公开密钥或私钥时发布必须失败。

- 普通分支 push、pull request、手动 dispatch 不触发发布。
- macOS ad-hoc identity `-` 不等于正式签名或公证。
- 不为测试工作流随意创建/推送 tag。
- 用户明确说“发布”且未指定版本时，以仓库最新有效 `vMAJOR.MINOR.PATCH` tag 为基准自动递增 patch 版本，同步应用版本后提交、推送、创建并推送新 tag；指定版本时使用指定版本。
- 发布 tag 成功推送到远端即完成代理侧发布动作；默认不等待 GitHub Actions 构建结束，后续失败由用户提出后再排查，除非用户明确要求持续跟踪。

## 10. 长期记忆与已知限制

- SQLite schema 26 与数据目录标记格式 1 是独立概念。
- 默认同步全部可选文件夹邮件头，正文按需；全文开关不是新调度入口。
- 每账户三条 IMAP 会话预算，完整同步只占两条。
- 搜索只覆盖当前账户/当前文件夹，不做跨账户或会话聚合。
- 自有通知窗口不等于系统通知中心。
- Vite 主入口仍有大于 500 kB 的 chunk 警告；不要把全局拆包混入无关阶段。
- 前端没有 ESLint/Prettier，也没有普通分支或 PR CI。
- 日志按日滚动，但没有保留/自动清理策略。
- 远程图片代理/缓存、CSS 背景图和 Web Font 未实现。
- Windows 正式代码签名与 Apple Developer 签名/公证仍未实现；updater 产物签名只用于应用内更新完整性，不能替代操作系统代码签名与公证。
- Linux 托盘的底层 AppIndicator 不提供图标点击事件，无法像 Windows/macOS 一样用单击直接恢复主窗口；左键打开菜单后选择“显示主界面”。

## 11. iteration、ADR 与文档维护

- `iterations/` 是历史入口：每份文件记录一次开发计划的范围、实施变更摘要、验证与验收。早期计划可能被后续计划取代，判断当前行为始终以本文和代码为准。
- 每次新的开发计划建立一份按 `YYYY-MM-DD-NN-主题.md` 命名的 iteration；后续实现批次直接追加到同一文件，不再分配全局阶段编号或创建独立 change 文档。
- ADR 只解释长期架构/安全理由，不是默认必读清单；索引见 [`adr/README.md`](./adr/README.md)。ADR 0001 作为已被取代的历史决策保留，当前单一 Tauri Rust package 边界以 ADR 0006 为准。
- 当前能力、技术栈、目录、数据格式、运行语义、限制或开发约定变化时更新本文。
- 重大架构/安全取舍新增 ADR；已有决定变化时更新状态和修订说明。
- 第三方资源或许可变化时更新 [`third-party-notices.md`](./third-party-notices.md)。
- 根 README 面向用户和贡献者，精确技术事实链接本文。
- 不再创建会话式交接文档、独立 change 流水账、重复技术参考或总体计划。
- 某次计划的详细实现由对应 iteration 与 Git 历史承担；只有跨计划继续有效的事实进入本文或 ADR。
