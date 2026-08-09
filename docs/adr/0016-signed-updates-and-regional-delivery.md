# ADR 0016：签名更新与区域化传输地址

状态：已采纳

日期：2026-08-09

修订：2026-08-09，补充独立更新窗口与不可信 Release notes 渲染边界。

## 背景

NextMail 需要在 Windows、macOS 和 Linux 的已安装应用内检测并安装新版本。GitHub Release 是现有唯一发布来源，但中国大陆到 GitHub Release 的清单和大文件下载可能不稳定。项目提供 Geo API 与 GitHub URL 反代，希望在不放宽更新完整性校验的前提下改善大陆下载。

Updater 会执行新二进制或安装包，因此更新来源属于高风险供应链边界。单纯切换下载域名、信任 HTTPS 或校验文件哈希都不足以替代由既有客户端内置密钥验证的发布签名。

## 决策

- 使用官方 Tauri Updater；所有更新产物必须由发布私钥签名，客户端使用构建时注入并固化进二进制的公开密钥验证。签名失败、密钥缺失或清单异常一律停止，不提供无签名降级。
- 私钥与可选密码只保存为 `TAURI_SIGNING_PRIVATE_KEY` / `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` GitHub Actions Secrets；公开密钥通过 `NEXTMAIL_UPDATER_PUBLIC_KEY` Repository Variable 注入发布构建。工作流在任一关键配置缺失时失败，不把密钥内容写入日志或仓库。
- GitHub Release 保留标准 `latest.json`。`tauri-action` 汇总各平台签名后可能写入 GitHub Release Assets API URL；发布完成前，工作流必须依据同一 Release 的资产元数据把它们规范化为公开 `browser_download_url` 并覆盖 `latest.json`，再派生 `latest-cn.json`，只为公开下载 URL 前置 `https://proxy.next-mail.app/`；平台映射、签名和版本元数据不变。
- 客户端以 4 秒上限请求 `https://api.next-mail.app/api/v1/geo`，响应中的 `ip` 与 `type` 仅为附加信息，区域判断只读取 `country_code`。明确返回 CN 时按代理 `latest-cn.json`、GitHub `latest.json` 排序；非 CN、超时、非成功状态或无效响应按 GitHub `latest.json`、代理 `latest-cn.json` 排序，使两种传输地址互为备用。地理结果不持久化、不写日志、不进入账户数据。
- React 只调用稳定的检查/安装 Command 并接收版本、可用性和发布说明 DTO；Geo 请求、清单选择、下载、签名验证、安装和重启均留在 Rust。前端不获得 updater、任意网络、Shell 或进程权限。
- 启动自动检查只提示可用版本，不静默安装；下载和安装必须由用户明确触发。
- 检测到新版本后，由 Rust 创建或复用单例独立更新窗口，并只向该窗口暴露读取当前可用更新的窄 Command。窗口使用独立最小 Capability，不获得任意网络、Shell、文件或建窗权限。
- Release notes 属于不可信清单数据：Rust 将长度限制为 64 KiB；前端只解析受限 Markdown，禁用原始 HTML、图片和危险协议。允许的绝对 HTTP(S) 链接必须继续通过 Rust 的外链校验边界后交给系统浏览器，WebView 不加载外部页面。

## 结果

代理故障或 Geo 服务故障不会阻止应用启动，且代理无法通过替换安装包绕过客户端签名。独立窗口隔离了更新提示的权限与生命周期，受限 Markdown 则允许保留发布说明结构而不引入任意 HTML。代价是每次正式发布前必须正确配置并保管同一套更新签名密钥；丢失私钥将导致现有客户端无法信任后续版本，只能重新安装。更换反代域名、签名信任根或放宽 Release notes 渲染能力必须作为安全变更审查并修订本 ADR。
