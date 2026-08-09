# 应用图标更新

日期：2026-08-09

## 状态

已验收。

## 范围

- 以用户绘制并放入仓库根目录的 `app-icon.png` 作为唯一图标源。
- 使用项目现有 Tauri 图标生成流程刷新桌面与移动端尺寸资产，包括 Windows `.ico`、macOS `.icns` 和各尺寸 PNG。
- 保持根 README 直接引用 `app-icon.png`，确保仓库首页展示新图标；核对托盘与安装包继续复用应用图标。
- 更新长期项目文档中与图标源和生成约定有关的事实。

## 非目标

- 不重新设计、裁切或调整用户提供的图形内容。
- 不改变产品名称、主题色、窗口布局、托盘交互、版本号或发布流程。
- 不提交、推送、创建 tag 或发布，除非用户后续明确要求。

## 验证门禁

- 源图为有效的正方形 PNG，包含适合桌面图标边缘的透明通道。
- Tauri 配置引用的所有图标文件成功刷新且格式可读取，README 图标路径仍有效。
- 执行必要的配置/资产检查、`pnpm build` 与 `git diff --check`；不运行 Tauri bundle。

## 实施结果

- 已确认用户提供的 `app-icon.png` 为 `1024×1024`、32-bit ARGB，画布四角透明且中心图形不透明；未修改其图形内容。
- 使用 `pnpm tauri icon app-icon.png` 刷新 `src-tauri/icons/` 下 Windows、macOS、通用 PNG、iOS 与 Android 全部现有图标资产。
- 根 README 继续直接引用 `./app-icon.png`；Tauri bundle 继续引用生成后的 PNG、`.ico` 与 `.icns`，托盘继续通过 `default_window_icon()` 复用同一应用图标。
- 长期项目文档已记录单一源图与统一生成命令，避免后续手工维护平台尺寸产生漂移。

## 验证结果

- 源图与 48 个生成 PNG 均可读取且保持正方形；Tauri 配置引用的 5 个 bundle 图标全部存在，`icon.ico` 与 `icon.icns` 文件签名有效。
- README 仍直接引用存在的 `app-icon.png`，托盘实现仍通过 `default_window_icon()` 复用应用图标。
- `pnpm build`：通过（仅保留项目既有的大 chunk 提示）。
- `git diff --check`：通过（仅有仓库现存行尾转换提示）。

## 验收结果

- 2026-08-09：用户确认验收通过并要求提交、推送。
