# 第二十二阶段：项目 README 与发布自动化

## 状态

已验收（2026-08-01，用户明确要求提交并发布 `v0.1.0`）。

## 目标

在不改变 NextMail 产品代码、依赖、数据格式或安全边界的前提下，重写面向 GitHub 访客的中英文项目 README，并建立只在推送 Git tag 时运行的 GitHub Actions 发布工作流，为 Windows、macOS 和 Linux 构建桌面安装产物并创建对应 GitHub Release。

## 已确认范围

### 项目 README

- 使用居中的 NextMail 标题、副标题和紧凑导航，提供中文与英文版本的页内切换入口。
- 添加适量 Shields.io 扁平方形徽章，优先展示版本、技术栈、目标平台和许可证等稳定信息。
- 重点介绍当前已经实现的产品特性，以及本地优先、安全边界、离线可用、渐进同步、持久化任务和原生桌面体验等设计理念。
- 避免展开内部模块、Command/Event、数据库表或同步实现等架构细节；详细技术信息继续链接到 `docs/`。
- 预留可直接替换的主界面、写信和深浅色等截图位置，不伪造尚未提供的产品截图。
- 不宣称尚未实现的统一收件箱、会话聚合、托盘、自动更新、签名、公证或 Linux 深度适配。

### GitHub Release 工作流

- 新增 GitHub Actions workflow，只响应推送的 Git tag，不响应普通分支 push、pull request 或手动 dispatch。
- 使用矩阵在 GitHub 托管的 Windows、macOS 和 Ubuntu runner 上构建当前 Tauri 2 应用。
- 使用仓库现有 pnpm lockfile 和 `src-tauri/Cargo.lock`，按单一 `src-tauri` Cargo package 边界安装与构建。
- 由 tag 名创建 GitHub Release，并上传三个桌面平台产生的 bundle；构建和发布使用最小必要 `contents: write` 权限。
- 不在本地执行 Tauri bundle，不实际创建 Release，不配置 Windows/Apple Developer 正式签名、Apple 公证、自动更新或额外发布渠道；macOS CI 只使用 Tauri 官方建议的 ad-hoc 签名兼容 Apple Silicon 下载产物。

## 非目标

- 不修改前端、Rust、数据库迁移、Capability、邮件协议或安全策略。
- 不新增或升级 Node/Rust/Python 依赖。
- 不增加分支 CI、PR 检查、自动版本改写、changelog 生成、nightly、beta 渠道或包管理器发布。
- 不承诺未正式签名或未公证的构建可绕过各平台系统安全提示，也不把 Linux 提升为深度适配或实机验收平台。
- 不 commit、push、创建 tag 或实际发布 GitHub Release。

## 实施顺序

1. 核对现有 README、项目版本、Tauri bundle 配置、许可证与仓库元数据。
2. 参考目标项目的信息层级，完成中英文 README、徽章和截图占位。
3. 依据 Tauri 与 GitHub 官方发布建议新增 tag-only 三平台 Release workflow。
4. 检查 YAML、Markdown 链接、现有配置兼容性和补丁格式，更新本阶段变更摘要。

## 自动验证

- 使用本机已安装的 YAML 解析器读取 `.github/workflows/release.yml`：通过；并断言顶层触发器只有 `push`、tag 模式只有 `v*`、矩阵恰好三个构建目标、公开 job 依赖完整 build 矩阵、构建阶段只创建草稿 Release。
- 扫描仓库 106 个 Markdown 文件中的 Markdown/HTML 本地链接：全部指向存在的文件或目录。
- `pnpm build`：通过；保留既有两个大于 500 kB 的 chunk 警告。
- `git diff --check`：通过；只有既有 Windows 工作区的 LF/CRLF 提示，没有空白错误。
- 没有运行 Tauri bundle、创建 tag 或实际 Release。

本阶段没有修改 React、Rust、依赖或应用行为，因此没有重复运行前端测试、Rust test/fmt/clippy；第二十一阶段的产品代码验证基线不受影响。

## 手动验收

1. 在 GitHub Markdown 预览中检查中英文切换、居中标题、徽章和截图占位的视觉层级。
2. 确认 README 只陈述当前能力，技术细节不过量，未实现与平台边界准确。
3. 检查 workflow 仅包含 tag push 触发，三个 runner 均安装 pnpm、Node、Rust 和对应 Linux 系统依赖。
4. 后续由用户自行决定是否创建测试 tag；本阶段不实际触发或验证线上 Release。

## 验收门禁

自动验证已经完成；用户已于 2026-08-01 明确要求提交、推送并发布 `v0.1.0`，本阶段验收通过。线上三平台 bundle 与 Release 结果由 `v0.1.0` tag 推送后的 GitHub Actions 继续验证。

## 迭代变更摘要

- 0060：重写中英文项目展示，增加 flat-square 徽章、截图占位和仅由版本 tag 触发的三平台预览 Release 工作流。
