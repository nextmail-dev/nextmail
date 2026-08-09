<div align="center">
  <img src="./app-icon.png" width="96" height="96" alt="NextMail 图标" />
  <h1>NextMail</h1>
  <p><strong>一款安静、可靠、本地优先的桌面邮件客户端。</strong></p>
  <p>快速离线阅读，忠实还原邮件，可靠完成投递——无需把收件箱交给另一朵云。</p>

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
> NextMail 当前处于 `0.2.2` 预览阶段。Windows 10 22H2+ x64 是主要实机验收平台，macOS 12+ 是目标平台。项目会为 Linux 生成早期测试包，但尚未对 Linux 进行深度适配或实机验收。

## 应用预览

<!-- 截图准备好后，用真实图片替换下面的单元格。建议路径：
     docs/screenshots/main-workspace.png
     docs/screenshots/composer.png
     docs/screenshots/appearance.png
-->

<table>
  <tr>
    <td colspan="2" align="center">
      <br />
      <strong>邮件工作区</strong><br />
      <sub>截图占位 · 主界面</sub>
      <br /><br />
    </td>
  </tr>
  <tr>
    <td width="50%" align="center">
      <br />
      <strong>富文本写信</strong><br />
      <sub>截图占位 · 新建、回复与转发</sub>
      <br /><br />
    </td>
    <td width="50%" align="center">
      <br />
      <strong>浅色与深色外观</strong><br />
      <sub>截图占位 · 主题与主题色</sub>
      <br /><br />
    </td>
  </tr>
</table>

## 特性亮点

| | |
| --- | --- |
| **📬 多账户管理**<br />添加、编辑、重新认证、切换并安全移除 IMAP/SMTP 密码账户。 | **⚡ 本地优先阅读**<br />先打开本地邮箱，再让后台同步逐步带来最新内容。 |
| **✍️ 完整写信体验**<br />富文本、HTML 源码、附件、内嵌图片、模板、签名、草稿、回复和转发。 | **🛡️ 安全且忠实的呈现**<br />保留常见邮件布局与内嵌图片，同时约束脚本、表单、危险链接和远程内容。 |
| **🔎 离线搜索**<br />在当前账户与文件夹内搜索主题、地址、预览、已下载正文和附件名。 | **🗂️ 真正的文件夹工作流**<br />无需离开桌面应用即可创建、重命名、移动、删除、排序文件夹和批量已读。 |
| **🔁 持久化操作**<br />已读、星标、移动、复制、删除、草稿与发件任务都能从中断中恢复。 | **🖥️ 原生桌面体验**<br />独立窗口、位置记忆、系统凭据库、桌面通知、双语界面与多套外观。 |

## 围绕收件箱，而不是云端

### 本地优先，网络在后

NextMail 把本地邮箱当作主要阅读界面。已有邮件无需等待网络往返即可出现，服务器工作则在后台继续。用户选择的数据目录可以整体迁移；账户密码始终留在操作系统凭据库中。

### 默认渐进，而非整批等待

同步会尽早交付有用内容：先收取邮件头，每封邮件完成本地提交后立刻进入列表。正文默认在打开时按需获取，只有用户为账户明确开启“收取邮件全文”后才会主动补齐。

### 在安全边界内追求保真

邮件 HTML 并不是普通网页。NextMail 尽量保留真实邮件依赖的布局、表格、作者样式、CID 图片与传统属性，同时以 Rust 权威清洗和 sandbox 阅读器把主动内容及未经允许的远程资源留在信任边界之外。

### 把失败当作状态，而不是数据丢失

邮件操作与待发送内容会先被可靠记录，再执行网络请求。重试复用持久化意图，而不是从界面状态重新猜测；SMTP 成功和 Sent 归档也彼此分离，避免归档失败导致同一封邮件被再次发送。

## 当前已经实现

- 基于密码的 IMAP/SMTP 账户、TLS、STARTTLS、自动发现，以及明文连接的双重明确确认。
- 全文件夹邮件头优先同步、可选全文同步、按需正文和基于本地原始 EML 的离线恢复。
- 已读/未读、星标、移动、复制、归档、删除、全部已读、文件夹管理与本地同层排序。
- 安全 HTML/CSS 与纯文本阅读、远程图片控制、CID/data 图片、原始 EML、附件下载、另存为和系统打开。
- 限定当前账户与文件夹的本地 FTS5 搜索。
- 基于 Tiptap/ProseMirror 与 CodeMirror 的富文本写信、显式草稿保存、Drafts/Sent 同步、模板、签名和变量。
- 回复、回复全部与转发，保留完整原始 HTML、内嵌图片、附件和稳定签名位置。
- 账户隔离的本地联系人、联系人建议、身份名片，以及邮件和联系人列表多选操作。
- 中文/英文、系统/浅色/深色外观、主题色、独立业务窗口与 NextMail 自有桌面通知。

准确的实现细节、工程约定与当前限制见[项目开发手册](./docs/project.md)。

## 下载

推送版本 tag 后，GitHub Actions 会为三个桌面平台构建 Release 产物：

| 平台 | 构建 | 当前支持状态 |
| --- | --- | --- |
| Windows 10 22H2+ | x64 安装包 | 主要实机验收目标 |
| macOS 12+ | Intel x64 与 Apple Silicon arm64 独立应用 | 目标平台；ad-hoc 签名，尚未公证 |
| Linux | 基于 Ubuntu 22.04 的 x64 安装包 | 实验性产物，不承诺深度适配 |

请从 [GitHub Releases](https://github.com/nextmail-dev/nextmail/releases) 下载已有版本。

> [!WARNING]
> 预览产物尚未使用正式的 Windows 代码签名或 Apple 公证，操作系统可能显示“开发者未经验证”等提示。请只从本仓库下载 NextMail。

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

在唯一的 Tauri package 中执行 Rust 验证：

```powershell
Push-Location src-tauri
cargo fmt --all -- --check
cargo test --offline --locked
cargo clippy --offline --locked --all-targets -- -D warnings
Pop-Location
```

Node.js 依赖只使用 pnpm 管理。项目目前不使用 Python；未来若引入 Python 工具，必须使用 uv。

## 项目文档

- [版本变更日志](./CHANGELOG.md)
- [项目开发手册](./docs/project.md)
- [阶段实施记录](./docs/iterations/)
- [架构决策记录](./docs/adr/)
- [第三方资源与许可证](./docs/third-party-notices.md)

## 当前边界

NextMail 尚未提供统一收件箱、会话聚合、跨账户搜索、托盘、系统通知中心集成、自动更新或正式签名与公证。当前预览版和发布工作流均不暗示这些能力已经存在。

## 许可证

NextMail Rust package 在 [`src-tauri/Cargo.toml`](./src-tauri/Cargo.toml) 中声明为 MIT License；第三方内容见[许可证说明](./docs/third-party-notices.md)。
