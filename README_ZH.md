<div align="center">
  <img src="./assets/app-icon.png" width="96" height="96" alt="NextMail 图标" />
  <h1>NextMail</h1>
  <p><strong>一款安静、可靠、本地优先的桌面邮件客户端。</strong></p>
  <p>先阅读本地邮件，渐进同步新内容，只在你需要时下载完整正文和附件。</p>

  <p>
    <a href="./README.md">English</a>
    ·
    简体中文
  </p>

  <p>
    <a href="https://github.com/nextmail-dev/nextmail/releases"><img alt="最新版本" src="https://img.shields.io/github/v/release/nextmail-dev/nextmail?display_name=tag&amp;label=release&amp;style=flat-square" /></a>
    <img alt="Tauri 2" src="https://img.shields.io/badge/Tauri-2-24C8DB?style=flat-square&amp;logo=tauri&amp;logoColor=white" />
    <img alt="React 19" src="https://img.shields.io/badge/React-19-149ECA?style=flat-square&amp;logo=react&amp;logoColor=white" />
    <img alt="Rust" src="https://img.shields.io/badge/Rust-stable-000000?style=flat-square&amp;logo=rust&amp;logoColor=white" />
    <img alt="Windows 与 macOS" src="https://img.shields.io/badge/platform-Windows%20%7C%20macOS-4C566A?style=flat-square" />
  </p>
</div>

> [!IMPORTANT]
> NextMail 当前处于预览阶段。Windows 10 22H2+ 和 macOS 12+ 是主要支持平台。Linux 提供早期测试包，尚未经过同等程度的适配和验收。

## 应用预览

![NextMail 工作区预览](./assets/screenshots/workspace/en_US-combined.png)

## 为什么选择 NextMail？

NextMail 把邮箱尽量留在你的设备上。已经保存到本地的邮件无需等待云服务即可打开；新邮件会在后台逐步到达。账户密码保存在操作系统的安全凭据库中，邮件数据则保存在你自己选择的目录里。

它适合希望邮件客户端启动快、节省网络流量，并且在网络不稳定时仍然好用的人。

NextMail 使用 **Tauri 2**、**React 19**、**TypeScript** 和 **Rust** 构建。桌面外壳与本地邮件服务由 Rust 提供，主要邮件工作区使用 React 呈现。

## 邮件同步规则：先同步信息，内容按需下载

NextMail 会把“展示邮箱所需的信息”和“邮件正文中的大块内容”分开处理。

1. **同步文件夹时只准备简短的文字预览，不下载完整正文。** 除了发件人、主题、日期、已读状态、星标状态和附件基本信息，还会取得少量文字，让每封新邮件进入列表时都有稳定高度的预览。
2. **不打开邮件，就不会从服务器下载它的完整正文。** 默认只自动取得列表所需的简短预览，浏览大型文件夹时不会悄悄下载每封邮件的全部内容。
3. **不打开或保存附件，就不会下载附件内容。** 邮件里显示了附件，不代表文件已经下载到本地。
4. **打开邮件时才按需获取阅读所需的正文。** 打开或保存附件时，只获取当前这个附件，其他邮件和附件保持不变。
5. **大型文件夹会逐步显示。** 新邮件到达后会立即出现在列表中，同时界面只保留当前视图需要的内容。即使文件夹有上万封邮件，也不会一次性生成上万个活跃界面元素。
6. **全文同步是可选项。** 只有你为某个账户主动开启后，NextMail 才会在后台补充下载邮件正文；默认仍然是打开时按需下载。

这样可以让邮箱尽快可用，同时减少不必要的下载、磁盘占用和后台工作。

## 主要特性

- **本地优先阅读** —— 已有邮件立即可读，网络中断时也能继续查看本地内容。
- **多账户管理** —— 添加、切换、编辑、重新认证和安全移除账户。
- **完整写信体验** —— 支持富文本、草稿、模板、签名、回复、转发、附件和内嵌图片。
- **离线搜索** —— 在当前账户和文件夹内搜索已经保存到本地的信息。
- **完整文件夹操作** —— 创建、重命名、移动、删除、排序文件夹，并批量标记已读。
- **可靠的邮件操作** —— 已读、星标、移动、复制、删除、草稿和发件任务在中断后可以继续处理。
- **安全且忠实的阅读** —— 尽量保留真实邮件的布局和内嵌图片，同时限制脚本、表单、危险链接和远程内容。
- **桌面应用体验** —— 独立窗口、位置记忆、桌面通知、主题、双语界面和签名更新检查。

## 谨慎处理邮件内容

邮件可能包含主动内容或危险链接。NextMail 会在显示前清理 HTML，远程图片由你控制；下载的附件也会经过检查后才允许打开或保存。

## 下载

请从 [GitHub Releases](https://github.com/nextmail-dev/nextmail/releases) 下载已有版本。

| 平台 | 状态 |
| --- | --- |
| Windows 10 22H2+ x64 | 主要实机验收平台 |
| macOS 12+ Intel 与 Apple Silicon | 支持目标；ad-hoc 签名，尚未公证 |
| Linux x64 | 用于早期测试的实验性产物 |

> [!WARNING]
> 预览版本尚未使用正式的 Windows 代码签名或 Apple 公证，操作系统可能显示“开发者未经验证”等提示。请只从本仓库下载 NextMail。

## 本地开发

请先安装 Node.js、pnpm、Rust stable，以及当前平台所需的 [Tauri 2 环境依赖](https://v2.tauri.app/start/prerequisites/)。

```powershell
pnpm install
pnpm tauri dev
```

在仓库根目录执行前端验证：

```powershell
pnpm test
pnpm build
```

在 `src-tauri` 中执行 Rust 验证：

```powershell
cargo fmt --all -- --check
cargo test --offline --locked
cargo clippy --offline --locked --all-targets -- -D warnings
```

实现细节、工程约定与当前限制见[项目开发手册](./docs/project.md)。

## 项目文档

- [版本变更日志](./CHANGELOG.md)
- [项目开发手册](./docs/project.md)
- [阶段实施记录](./docs/iterations/)
- [架构决策记录](./docs/adr/)
- [第三方资源与许可证](./docs/third-party-notices.md)

## 当前边界

NextMail 当前尚未提供统一收件箱、会话聚合和跨账户搜索。Linux 仍属于实验性平台，Windows 正式代码签名和 Apple 公证也尚未提供。

## 许可证

NextMail Rust package 在 [`src-tauri/Cargo.toml`](./src-tauri/Cargo.toml) 中声明为 MIT License；第三方内容见[许可证说明](./docs/third-party-notices.md)。
