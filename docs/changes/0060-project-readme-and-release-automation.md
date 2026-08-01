# 变更 0060：项目 README 与发布自动化

日期：2026-08-01

## 项目展示

- 根 README 改为居中的 NextMail 品牌区、英文主版本与简体中文切换入口，并增加独立 `README_ZH.md` 中文版本。
- 两个版本统一使用少量 Shields.io `flat-square` 徽章，展示 Release、Tauri 2、React 19、Rust stable 和主要目标平台。
- README 重点说明多账户、本地优先阅读、安全保真、富文本写信、离线搜索、文件夹管理、持久化操作与原生桌面体验，不再展开内部 Command/Event、模块或表结构。
- 以一个主界面和两个并列区域预留邮件工作区、富文本写信和浅/深色外观截图位置，并在注释中约定后续截图建议路径；当前不伪造产品截图。
- 下载与平台表明确 Windows 是主要实机验收目标、macOS 是目标平台、Linux bundle 为实验性产物，并说明正式 Windows 代码签名与 Apple 公证尚未实现。

## Tag 驱动的 GitHub Release

- 新增 `.github/workflows/release.yml`，只响应 `v*` tag push，不响应普通分支 push、pull request 或手动 dispatch。
- 矩阵使用 `windows-latest` 构建 Windows x64、`ubuntu-22.04` 构建 Linux x64，并在 `macos-latest` 同时安装 `aarch64-apple-darwin` 与 `x86_64-apple-darwin` target，生成 macOS Universal bundle。
- Node 依赖使用 pnpm 10 和仓库 `pnpm-lock.yaml` 的 frozen install；Rust 继续使用唯一 `src-tauri` package，并把 `--locked` 传给 Cargo。
- Linux 安装 Tauri 2 官方构建依赖；macOS 使用 ad-hoc identity `-`，不配置 Apple Developer 证书或公证。
- 各矩阵任务通过 `tauri-apps/tauri-action` 把 bundle 上传到同一个草稿 Release；只有三个平台全部成功，最终 job 才使用 GitHub CLI 转为公开 Release。任一平台失败时不会公开半套产物。
- 工作流仅授予创建/更新 Release 所需的 `contents: write`；没有产品凭据、签名秘密、自动更新配置或额外发布渠道。

## 边界

- 没有修改 React、Rust、依赖、数据库、Capability、协议、同步或邮件内容安全策略。
- 没有在本地执行 Tauri bundle，没有创建 tag 或实际 GitHub Release，也没有 commit 或 push。
- 正式签名、公证、自动更新、分支/PR CI 和 Linux 深度适配仍需未来单独确认。

## 验证

- `.github/workflows/release.yml` 经 YAML 解析通过，并确认只有 `v*` tag push、三个构建目标和 build 成功后的发布依赖。
- 仓库 106 个 Markdown 文件的本地 Markdown/HTML 链接全部可解析到现有文件或目录。
- `pnpm build` 通过，保留既有大 chunk 警告。
- `git diff --check` 通过。
- 因为没有产品代码或依赖变化，不重复运行前端测试和 Rust test/fmt/clippy；按阶段边界未运行 Tauri bundle。
- 用户已于 2026-08-01 明确要求提交、推送并发布 `v0.1.0`，第二十二阶段验收通过；线上 bundle 由 tag 工作流执行。
