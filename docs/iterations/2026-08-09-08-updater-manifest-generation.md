# Updater 清单自行生成

日期：2026-08-09

## 状态

已验收。

## 背景

`v0.3.0` 的四平台构建完成后，发布任务在 `Normalize updater manifests` 中通过 Tag 下载草稿 Release 的 `latest.json`。GitHub 的按 Tag Release 查询只返回已发布 Release，导致 `gh release download` 在清单规范化阶段返回 HTTP 404，而 Release 又必须等清单处理完成后才能公开。

## 范围

- 参考 `cc-switch` 的发布流程，由 NextMail 工作流根据各平台实际 updater 产物和 `.sig` 自行生成 `latest.json`。
- 保留 `tauri-action` 负责四个平台的构建、签名以及草稿 Release 资产上传，但关闭其自动 updater JSON 上传。
- 通过 GitHub Actions workflow artifact 汇总各构建任务产生的 updater 签名。
- 最终任务生成直连 `latest.json` 与代理 `latest-cn.json`，完整验证后上传，并在同一步公开草稿 Release。
- 按用户指示删除本地与远端旧 `v0.3.0` Tag，在包含新应用图标和本次发布工作流修复的新 HEAD 上重新创建并推送 `v0.3.0`，以实际 Release workflow 验证完整链路。

## 非目标

- 不修改应用版本、客户端 updater endpoint、签名密钥或平台构建矩阵。
- 不修改应用运行时代码或增加新的运行时依赖。
- 不删除或改写用户未明确授权的其他 Tag、Release 或提交历史。

## 验证门禁

- 标准清单版本与当前 Tag 一致，发布说明来自对应 `CHANGELOG.md` 版本段落。
- 清单严格包含当前支持的 11 个 Tauri 平台键，并为每项提供非空签名和当前 Tag 的公开下载 URL。
- 代理清单的平台、版本、说明、发布日期和签名与标准清单一致，只对下载 URL 前置 `https://proxy.next-mail.app/`。
- 任一必要签名缺失、重复或 URL/平台映射不符合预期时，Release 保持草稿且工作流失败。
- Workflow YAML、清单生成脚本和最终 Git 差异通过静态验证。
- 重新推送 `v0.3.0` 后，四个平台构建、清单生成、Release 发布及公开资产检查全部通过。

## 实施结果

- `tauri-action` 继续构建、签名并向草稿 Release 上传原有四平台产物，但通过 `uploadUpdaterJson: false` 停止上传自动清单。
- 每个矩阵任务收集其本地 `.sig` 并作为保留 1 天的独立 workflow artifact 上传；同名签名会直接使任务失败，不会静默覆盖。
- 新增无第三方运行时依赖的 Node.js 清单生成器，根据当前 Tauri 真实产物命名识别 macOS arm64/x64、Linux AppImage/deb/rpm 和 Windows MSI/NSIS 签名，并生成兼容当前客户端的 11 个平台键。
- 生成器严格校验 Tag、仓库名、发布时间、发布说明、签名内容、平台全集及直连/代理 URL 对应关系，任何必要签名缺失或重复都会阻止发布。
- 最终任务使用 `softprops/action-gh-release@v3` 将两个清单上传至既有草稿 Release，并在资产上传完成后公开，移除了全部按 Tag 下载草稿 Release 的 `gh` 调用。
- 首次重新运行在最终任务提取 Changelog 时暴露了 AWK 正则字面量的错误转义；两个提取步骤统一改用 `index()` 判断版本标题，避免字符串与正则字面量使用不同转义规则。
- 第二次重新运行确认两个 macOS 构建的本地签名都叫 `NextMail.app.tar.gz.sig`，与 Release 上带架构的资产名不同，并会在 artifact 合并时同名覆盖。构建任务现在按矩阵架构将其规范化为 `NextMail_<version>_aarch64.app.tar.gz.sig` 或 `NextMail_<version>_x64.app.tar.gz.sig` 后再汇总；Linux 与 Windows 保留已经与 Release 一致的原名。

## 验证结果

- `node --check .github/scripts/generate-updater-manifests.mjs`：通过。
- `node --check .github/scripts/generate-updater-manifests.test.mjs`：通过。
- `node --test .github/scripts/generate-updater-manifests.test.mjs`：通过，使用与公开 `v0.2.3` 资产一致的七类文件名验证 11 个平台键、直连 URL、代理 URL 和 JSON 落盘结果。
- `go run github.com/rhysd/actionlint/cmd/actionlint@v1.7.12 .github/workflows/release.yml`：通过。
- `git diff --check`：通过，仅有仓库既有的行尾转换提示。
- 未在本地运行产品构建或 Tauri bundle；跨平台构建与草稿 Release 发布链由 GitHub Actions 实际验证。
- 首次远端运行的四个平台构建通过，最终任务在 `Extract changelog entry` 因 `unterminated regexp` 失败；已针对该根因修复。
- 第二次远端运行通过 Changelog 提取，但生成清单时因合并后的签名中不存在可识别的 macOS arm64 文件名而失败；已从运行 `31315051256` 的四个 workflow artifacts 核对全部实际文件名并修复 macOS 规范化。
- 2026-08-11：修正后的 `v0.3.0` 四平台构建、Changelog 提取、11 平台 updater 清单生成、直连/大陆代理 URL、Release 公开发布及最终产物均通过，用户确认全部验收通过。
