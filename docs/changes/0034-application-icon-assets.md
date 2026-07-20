# 0034 应用图标资产

日期：2026-07-20

## 变更

- 新增仓库根目录 `app-icon.png`，作为 1024 × 1024、带透明通道的 NextMail 应用图标源文件。
- 由该源文件更新 Tauri 的 PNG、Windows ICO/Store Logo 和 macOS ICNS 图标。
- 保留图标生成工具输出的 Android adaptive icon 与 iOS AppIcon 尺寸资产，便于 Tauri 平台资产保持同源。

## 行为影响

本次只替换应用品牌图标资源，不修改应用代码、依赖、配置权限、窗口行为、数据格式或邮件功能。Windows 与 macOS 安装包继续引用 `tauri.conf.json` 中既有的桌面图标路径。

## 验证

- 人工检查 1024 × 1024 源图标，确认透明通道与图形内容正常。
- 逐一解码生成的 48 个 PNG，确认尺寸有效且文件可读取。
- 确认 `tauri.conf.json` 引用的 PNG、ICNS 与 ICO 文件全部存在。
- `git diff --check` 通过。

未执行 Tauri bundle；图标的安装包和操作系统缓存效果留待后续正常打包或开发运行时观察。
