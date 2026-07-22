# 0041 Windows 安装程序语言选项

日期：2026-07-22

## 变更

- Tauri NSIS 安装程序配置显式包含简体中文、繁体中文和英文。
- 启用 NSIS 语言选择器，使安装程序和卸载程序窗口显示前允许用户选择上述语言。
- 系统语言仍作为默认选择；无法匹配时由 Tauri/NSIS 回退到配置列表中的首个语言。

## 保持不变

- 不修改 NextMail 应用内的中文/英文语言资源、首次启动语言逻辑或系统区域设置。
- 不修改 Windows 运行时窗口、邮件功能、权限、数据格式、自动更新、签名或发布流程。
- macOS 和其他平台的 bundle 配置保持不变。

## 验证

- 对照当前锁定的 Tauri CLI 配置 schema，确认 `languages` 与 `displayLanguageSelector` 位于有效的 `bundle.windows.nsis` 配置节点。
- `tauri.conf.json` 可正常解析，`pnpm tauri info` 接受当前配置，`git diff --check` 通过。
- 未执行 Tauri bundle；三种语言的安装/卸载界面留待后续正式打包时实机验证。
