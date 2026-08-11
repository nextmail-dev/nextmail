# UI 视觉重构

状态：已验收

## 目标

在不改变邮件业务能力、桌面安全边界和平台差异的前提下，建立清晰、紧凑且具有桌面质感的统一视觉系统，并分批覆盖主窗口、Composer、设置、账户管理及其他独立窗口。

## 范围

- 建立亮色与深色主题共用的表面、边界、高光、阴影和交互状态令牌。
- 恢复具有明确边界和窗口标题的自绘标题栏，同时保留 Windows 控件与 macOS 原生交通灯。
- 统一 Button、输入框、选择器、菜单、对话框、滚动条和分栏拖动条等基础组件的材质。
- 收紧主窗口侧栏、邮件列表和阅读区的信息密度，明确三个工作区的永久边界。
- 在后续批次统一 Composer、设置、账户管理、定义编辑器、邮件预览和更新窗口。
- 完成亮色、深色、最小窗口尺寸及 Windows/macOS 差异的实机验收。

## 非目标

- 不新增邮件标签、筛选器、统一收件箱或参考图中不存在于当前产品的数据能力。
- 不改变 Tauri Command、DTO、同步、存储、邮件清洗或 iframe 安全边界。
- 不为展示稿外层阴影改用透明窗口，也不以自绘按钮替代 macOS 原生交通灯。
- 不一次性重写全部页面或预先建立未被当前批次消费的组件抽象。

## 分批实施

### 第一批：视觉基础

- 主题表面、边界、高光与阴影令牌。
- 明确标题栏、永久分栏边界。
- 核心 UI 基础组件材质与状态统一。

### 第二批：主工作区

- 账户切换与文件夹侧栏。
- 邮件列表密度、选中和未读状态。
- 阅读区扁平化及标题、发件人、操作、正文、附件层级。

### 第三批：独立业务窗口

- Composer。
- 设置与账户管理。
- 定义编辑器、邮件预览、原文、通知和更新窗口。

### 第四批：一致性与实机验收

- 亮色与深色主题对比度。
- 最小窗口尺寸、长文案与多 DPI。
- Windows 自绘控件与 macOS 原生交通灯。

## 验证门禁

- 第一批及后续每批运行相关 Vitest。
- 每个代码批次运行 `pnpm build` 与 `git diff --check`。
- 主工作区批次完成后通过 `pnpm tauri dev` 在 Windows 实机检查亮色、深色、窗口缩放和拖动分栏。
- macOS 行为未经实机验证时不得宣称通过。

## 第一批实施结果

状态：已验收

- 重建亮色、深色与跟随系统主题的表面色阶，补充强边界、标题栏、高光以及 control / raised / float / overlay 阴影令牌。
- 应用底层与标题栏使用轻微纵向材质渐变；主操作渐变调整为 135°，但继续由用户选择的主题色推导，不锁死展示稿的紫色。
- 自绘标题栏由透明拖动区恢复为 42px 独立表面，增加永久下边界和居中业务窗口标题；保留 Windows 自绘控制与 macOS 原生交通灯分支。
- 分栏拖动线改为常驻强边界，hover 与键盘 focus 时使用主色加粗；删除仅为旧标题栏渐变同步侧栏宽度的 DOM 副作用。
- Button、Surface、输入框、复选框、选择器、菜单、对话框、搜索框、Switch 和 Toast 统一使用语义边界、高光与阴影；主按钮保留明暗反馈但不再产生 hover 位移。
- 第一批手动检查反馈中，邮件行增加固定左右内缩的 1px 分割线；滚动条绝对覆盖在右侧边距中，不参与列表宽度计算，出现或消失都不改变邮件行、内容或分割线的位置。邮件列表滑块仅在 hover 或键盘 focus 时显示，宽度增至 6px 并使用默认指针。阅读区移除面板内嵌的大卡片外框，恢复为完整平面工作区。
- 除上述列表分割线与阅读区扁平化外，未调整主窗口信息架构、列表密度或阅读内容层级，这些仍属于第二批。

验证：

- `pnpm test src/styles/base.test.ts src/components/ui/resize-handle.test.tsx src/features/mail/hooks/usePaneLayout.test.tsx`：8 项通过。
- `pnpm test src/features/mail/MessageListPane.test.tsx src/features/mail/MessageViewer.test.tsx`：15 项通过。
- `pnpm test src/components/ui/overlay-scroll-area.test.tsx src/features/mail/MessageListPane.test.tsx`：10 项通过。
- `pnpm test src`：40 个测试文件、137 项测试全部通过。
- `pnpm build`：通过；仅保留既有的大 chunk 提示。
- `git diff --check`：通过。

## 第二批实施结果

状态：已验收

- 账户区头像、文字、菜单项和上下留白收紧；文件夹侧栏统一为 36px 行高，写邮件按钮收紧为 40px，联系人、文件夹与设置使用一致的紧凑节奏。
- 文件夹与联系人选中态增加 2px 左侧主色标记；文件夹未读数改为轻量主色数字，避免多个高饱和色块争夺注意力。
- 邮件列表标题与邮件标题统一为 18px；列表头、搜索框及邮件行垂直间距收紧，选中标记改为 2px。未读邮件使用轻微主色底与较强字重，已读邮件降低字重和文字对比度。
- 阅读区将邮件标题与完整操作工具栏合并到同一顶栏，发件人、地址、日期与附件摘要作为第二层；窄宽度下工具栏自动换行，正文和附件继续保持独立边界，未恢复内嵌大卡片。
- 联系人列表增加固定左右内缩 20px 的 1px 分割线，最后一项不绘制；分割线坐标不受滚动条出现与否影响。
- `OverlayScrollArea` 改为全局强制 overlay 行为：6px 滑块绝对覆盖右侧，不占内容宽度，仅在 hover 或键盘 focus-within 时显示并使用默认指针。清理现有滚动容器为滑块预留的非对称右内边距，并同步更新 `AGENTS.md` 与 `docs/project.md`，使未来滚动容器默认遵守同一规则。
- 文件夹列表按圆角非全宽项目的实际布局作唯一位置例外：展开侧栏的滚动容器向右延伸到侧栏留白，列表内容使用等量固定右边距维持项目宽度，使滑块位于圆角项外侧且不侵占项目。

验证：

- `pnpm test src/features/mail/AccountSwitcher.test.tsx src/features/mail/MailboxPane.test.tsx src/features/mail/MessageListPane.test.tsx src/features/mail/MessageViewer.test.tsx`：4 个测试文件、28 项测试通过。
- `pnpm test src/components/ui/overlay-scroll-area.test.tsx src/features/mail/MailboxPane.test.tsx src/features/contacts/ContactsWorkspace.test.tsx src/features/accounts/AccountManagement.test.tsx src/features/composer/RichTextEditor.test.tsx src/features/preferences/NotificationSettings.test.tsx`：6 个测试文件、31 项测试通过。
- `pnpm test src`：40 个测试文件、136 项测试全部通过。
- `pnpm build`：通过；仅保留既有的大 chunk 提示。

## 第三批实施结果

状态：已验收

- Composer 将发送操作、收件信息、模板/签名、附件与编辑工具栏改为连续但边界明确的分层表面，收紧顶栏与选择区高度；输入、草稿、发送和窗口关闭行为保持不变。
- 设置窗口收紧侧栏和导航行，增加永久分栏边界与选中标记；内容标题避免继承过大的页面字号，设置分组统一使用语义边界、raised 阴影和低对比表面。
- 账户管理窗口拆分独立标题区和内容区，账户列表增加明确容器边界，当前账户使用与主工作区一致的主色左标记；账户管理状态与表单逻辑保持不变。
- 定义编辑器统一标题、收件信息、富文本编辑器和底部操作区的边界；原文窗口与更新窗口使用独立标题/内容层级，通知窗口补充窗口边界和内侧高光。
- 邮件预览窗口继续复用第二批已经扁平化的 `MessageViewer`，不建立独立视觉分支。
- 全局可点击控件统一使用桌面默认指针，不再显示手型 pointer；文本选择、窗格缩放和拖动继续保留相应的语义指针。源码中不再存在 `cursor-pointer`、`cursor: pointer` 或主按钮 hover 位移规则。

验证：

- `pnpm test src/styles/base.test.ts src/components/ui/button.test.tsx src/features/composer/ComposerApp.test.tsx src/features/preferences/SettingsApp.test.tsx src/features/accounts/AccountManagement.test.tsx src/features/preferences/CompositionDefinitionEditorApp.test.tsx src/features/mail/RawMessageApp.test.tsx src/features/notifications/NotificationApp.test.tsx src/features/preferences/UpdateWindowApp.test.tsx`：9 个测试文件、34 项测试通过。
- `pnpm test src`：40 个测试文件、137 项测试全部通过。
- `pnpm build`：通过；仅保留既有的大 chunk 提示。
- `git diff --check`：通过；指针规则扫描未发现 `cursor-pointer`、`cursor: pointer` 或 hover 位移。

## 第四批实施结果

状态：已验收

- 保留用户保存的主题源色，在统一外观入口根据当前亮色/深色模式校正实际主色明度，并同步切换黑色或白色主色前景；亮色琥珀、绿色以及深色蓝/紫等原先低对比组合不再直接用于界面文字和主操作。跟随系统主题时监听明暗切换并重新计算，不修改持久化偏好。
- 亮色次要文字调整到能在 card、sidebar、input 等主要表面保持清晰的色阶；主题色选择器用实际校正后的主色呈现当前选项。
- 通用标题、标签与正文增加超长连续内容换行；联系人详情标题在窄阅读区固定为 24px，身份标签限制为容器宽度，避免长姓名、邮箱或无空格主题撑破布局。
- Composer、设置关闭确认、账户、定义编辑器、更新、联系人、文件夹与首次使用等操作按钮组允许换行，最小窗口与长文案组合下不隐藏关键操作。
- 主窗口补充 920px 最小逻辑宽度回归：文件夹 238px、列表 310px、阅读区及分隔线预算 372px。现有通知定位继续按显示器 scale factor 将逻辑尺寸转换为物理像素。
- 窗口标题栏只在 Windows 呈现自绘最小化/最大化/关闭按钮及双击最大化；macOS 保留原生交通灯，其他带系统装饰的平台不再重复显示 Windows 控件。
- 深色主题从蓝灰表面改为中性黑灰层级，主色只保留在操作、选中、焦点和状态反馈；主题模式预览同步改为中性深色示意。
- 保留键盘可访问性，但将普通按钮、文件夹/联系人等列表行的 `focus-visible` 从抢眼粗框收敛为 1px 内描边；输入和选择表面使用较弱的 2px 内反馈，鼠标操作不会显示该键盘焦点态。
- 联系人列表行由 4 单位垂直内边距和 40px 头像收紧为 3 单位与 36px，分割线及滚动条几何规则保持不变。
- 阅读区顶栏仅保留星标、回复、回复全部、转发、归档、删除和更多等高频入口，移动与复制邮箱操作合并到更多菜单的分级菜单；原有目标邮箱滚动能力保留。
- Composer 工具栏保留字体、颜色、基础字形、列表和撤销/重做，引用、链接、移除链接、行内图片及 HTML 源码归入“更多格式选项”，不再依赖工具栏横向滚动。
- 标题栏监听窗口 focus / blur；失焦时只降低标题和 Windows 自绘控件强调度，不隐藏永久边界。

验收：

- `git diff --check`：通过；中英文 locale JSON 解析通过。
- 用户已在 Windows 实机验收亮色/深色、焦点态、滚动条、列表密度、标题栏和工具栏调整，确认本阶段验收通过。
- 按用户明确要求，本批不再补跑 `pnpm test` 与 `pnpm build`；第三批结束时最近一次完整结果仍为 40 个测试文件、137 项测试全部通过且生产构建通过。
- macOS 原生交通灯与 Overlay 行为未在 macOS 实机单独验证，不宣称该平台已通过实机测试；本阶段未改变既有 macOS 原生控制边界。
