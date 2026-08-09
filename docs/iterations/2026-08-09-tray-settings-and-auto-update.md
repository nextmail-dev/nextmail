# 托盘、设置分组与自动更新

日期：2026-08-09

## 状态

已验收。

## 范围

- 为 Windows、macOS 与 Linux 增加系统托盘；左键单击显示主窗口，右键菜单提供“显示主界面”“设置”“退出”。
- 新增设备级桌面偏好：最小化到托盘默认关闭、退出时询问默认启用。关闭主窗口时按偏好直接隐藏或退出；需要询问时展示包含“不再询问”的选择框，“不再询问”会保存本次选择并关闭后续询问。
- 按职责整理设置页内容；“更多”中的关闭偏好归入“托盘图标”，现有列表行为归入独立分组，其余页面按信息、内容或行为分组。
- 修正首次设置界面顶部重复预留标题栏高度造成的空白，同时保留 Windows 窗口控件与 macOS 原生交通灯安全区。
- 使用 Tauri Updater 实现签名更新检查和安装；启动时自动检查可配置，“关于”页提供手动检查入口与明确状态反馈。
- 更新检查先请求 `https://api.next-mail.app/api/v1/geo`；仅当明确返回 `CN` 时使用代理清单，其余情况（包括定位失败）使用 GitHub 直连清单。大陆清单中的安装包 URL 也由发布工作流改写为代理 URL，避免只代理清单而仍直连附件。
- 发布工作流为 updater 产物生成签名与 `latest.json`，并额外生成 `latest-cn.json`；继续只由 `v*` tag 触发发布。

## 已确认行为

- 托盘菜单中的“退出”是明确退出操作，不再二次询问。
- “退出时询问”关闭后，“最小化到托盘”决定关闭主窗口时隐藏还是退出。
- 在关闭询问中勾选“不再询问”时，本次选择会同步写入“最小化到托盘”，并将“退出时询问”关闭。
- 启动时自动检查只提示可用版本，不静默下载安装；安装仍需用户明确确认。
- 自动检查更新默认启用；网络或定位失败不得阻止应用启动，也不得绕过 updater 签名校验。

## 安全与交付约束

- React 不获得网络、Shell、进程或 updater 插件权限；地理查询、清单选择、签名校验、下载和安装统一封装在 Rust Command 后面。
- Updater 签名验证不得禁用。仓库只记录公开验证密钥的配置方式；私钥与密码只由 GitHub Actions Secrets 注入，不写入仓库或日志。
- 代理只改变可信清单和产物的传输地址，不改变签名信任根；定位结果不写日志、不持久化、不作为账户数据。
- 不改变现有窗口 Capability、邮件网络边界、数据库迁移或账户隔离。
- 本计划不包含提交、推送、打 tag 或发布；需用户验收后另行明确要求。

## 验证门禁

- 托盘左键、三项菜单、主窗口关闭询问、隐藏/恢复、明确退出及偏好持久化均有针对性测试或可复现实机步骤。
- 设置页的分组、中文与英文文案完整；首次设置界面在 Windows/macOS 标题栏模式下均无额外空白且控件不遮挡内容。
- Geo 返回 CN、非 CN、超时和无效响应分别落入预期路由；所有更新结果均映射为稳定 DTO，不向 UI 返回内部 URL、路径或原始网络错误。
- 发布工作流只在 tag push 触发，并为各平台 updater 产物生成直连与大陆代理清单；缺失签名 Secrets 时明确失败，不退化为无签名更新。
- 执行 `pnpm test`、`pnpm build`、`cargo fmt --all -- --check`、`cargo test --offline --locked`、`cargo clippy --offline --locked --all-targets -- -D warnings` 和 `git diff --check`；不运行 Tauri bundle。

## 实施结果

- Rust 创建单一 `nextmail-tray` 托盘实例并复用应用图标；菜单随中英文偏好即时切换，提供显示主界面、设置与明确退出。Windows/macOS 左键直接显示、取消最小化并聚焦主窗口；Linux 因 Tauri/AppIndicator 不产生图标点击事件，左键显示菜单后使用“显示主界面”。托盘初始化失败不会阻止应用启动，也不会允许把主窗口隐藏成无法恢复的状态。
- 新增独立 `desktop-preferences.json` 与窄 Store/port/service/Command，默认 `minimizeToTray=false`、`askBeforeExit=true`、`autoCheckUpdates=true`。Rust 统一拦截主窗口关闭：需要询问时只发窄事件，React 展示最小化、退出和“不再询问”；记住选择时同步保存关闭动作并关闭后续询问。托盘菜单的退出保持明确退出，不重复弹窗。
- “更多”按“托盘图标”“列表行为”分组；阅读页拆分“内容与隐私”“附件”“列表行为”，通用、外观与关于页也使用对应职责分组。关于页新增启动自动检查与手动检测按钮、最新版本状态、更新说明以及下载安装确认。
- 首次设置右侧页面不再重复预留标题栏高度，且向主标题栏注入与页面一致的背景令牌；左侧 Windows 控件/macOS 交通灯安全区保持不变。
- Rust 使用 `tauri-plugin-updater` 动态构造更新器。Geo 请求限制为 4 秒，只有明确 `CN` 走代理 `latest-cn.json`，其他响应与所有失败回退 GitHub `latest.json`；对 React 只返回版本、可用性和发布说明，不返回 URL、路径或原始网络错误。
- Release workflow 启用 updater artifacts，在构建时由 Repository Variable 注入公开验证密钥、由 Secrets 注入签名私钥和密码；缺少公开密钥或私钥时立即失败。四平台完成后由 `latest.json` 生成只改写 GitHub 下载 URL 的 `latest-cn.json`，再公开 Release。
- 补齐 `plugins.updater` 的公开验证密钥与 GitHub 直连 endpoint，满足插件在 dev 与 release 启动阶段的配置反序列化要求；运行时仍按 Geo 结果覆盖为直连或大陆代理清单，签名私钥只来自 GitHub Secrets。
- 新增 [ADR 0016](../adr/0016-signed-updates-and-regional-delivery.md)，长期固定签名信任根、区域路由和 React/Rust 权限边界。

## 自动验证（2026-08-09）

- `pnpm test`：35 个测试文件、124 项测试通过。
- `pnpm build`：通过；仅保留既有大 chunk 警告。
- `cargo check --offline --locked`：通过。
- `cargo test --offline --locked`：163 项测试通过。
- `cargo clippy --offline --locked --all-targets -- -D warnings`：通过。
- `cargo fmt --all -- --check` 与 `git diff --check`：通过。
- 实际只读核对 Geo API 返回约定 JSON、反代首页返回 HTTP 200。
- 用户已完成手动验收；本计划随 `v0.2.2` 发布。

## 手动验收建议

1. Windows 与 macOS 分别验证托盘左键恢复主窗口，右键三项菜单可用；切换中英文后菜单立即更新。Linux 验证左右键菜单和“显示主界面”，不要求底层不支持的图标点击事件。
2. 保持默认设置关闭主窗口，分别测试取消、最小化到托盘、退出；勾选“不再询问”后确认设置值与所选动作一致，重启后仍持久化。关闭“退出时询问”后分别切换“最小化到托盘”，确认关闭动作直接隐藏或退出。
3. 在首次设置三个页面检查顶部不再出现与右侧页面不同色的空白带，Windows 控件与 macOS 交通灯不遮挡标题或输入区。
4. 浏览通用、外观、阅读、更多和关于，确认设置项分组层级、中英文、保存失败提示与现有选项行为。
5. 第一次签名发布前，在仓库外生成并妥善备份长期 updater 密钥；把公钥配置为 `NEXTMAIL_UPDATER_PUBLIC_KEY` Repository Variable，把私钥和可选密码配置为 `TAURI_SIGNING_PRIVATE_KEY`、`TAURI_SIGNING_PRIVATE_KEY_PASSWORD` Secrets。丢失私钥会使既有客户端无法更新，不得把私钥提交仓库。
6. 使用高于当前版本的测试 Release 验证：非 CN 获取 `latest.json`，CN 获取代理 `latest-cn.json`；手动与启动检查均显示相同版本，确认后下载、签名验证、安装并重启。篡改签名或产物必须安装失败。
