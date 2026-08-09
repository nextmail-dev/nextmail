# Updater 清单 URL 规范化

日期：2026-08-09

## 状态

已验收。

## 范围

- 保留 `tauri-action` 生成的平台映射与签名，在 Release 公开前把资产 API URL 规范化为公开 `browser_download_url`。
- 从规范化后的 `latest.json` 派生 `latest-cn.json`，只为下载 URL 增加 NextMail 反代前缀。
- 为已经发布的 `v0.2.3` 生成可手动覆盖上传的两个清单文件。

## 非目标

- 不重建或重新签名 `v0.2.3` 安装包，不修改客户端版本、签名信任根或 updater endpoint。
- 本次不提交、推送、打 tag 或重新发布，除非用户后续明确要求。

## 验证门禁

- 两个清单的平台键、版本、说明、发布日期与签名保持一致。
- `latest.json` 的所有下载 URL 使用当前 tag 的 GitHub Release 公共下载地址。
- `latest-cn.json` 的所有下载 URL 是对应公共下载地址加 `https://proxy.next-mail.app/` 前缀。
- Workflow 在覆盖上传两个清单前验证平台非空、签名非空和 URL 前缀，再公开 Release。

## 实施结果

- Release 发布任务现在读取当前 tag 的资产元数据，把 `tauri-action` 生成清单中的 Assets API URL 映射为对应的 `browser_download_url`。
- 规范化后的 `latest.json` 与由其派生的 `latest-cn.json` 会一并覆盖上传；发布前同时验证 tag 对应版本、平台非空、签名非空和直连/代理 URL 前缀。
- 已基于公开的 `v0.2.3` Release 资产和原始 Tauri 签名生成两份手动修复清单，未重建或重新签名安装包。

## 验证结果

- 公开 `v0.2.3` Release、四个平台构建任务和最终发布任务均已确认成功。
- 两份 JSON 均可解析，版本为 `0.2.3`，包含相同的 11 个 Tauri 平台键。
- 逐平台比对 Release 原始 `latest.json`：全部签名保持不变且非空。
- `latest.json` 的全部 URL 均匹配 `v0.2.3` 真实 Release 资产；`latest-cn.json` 的全部 URL 均严格等于反代前缀加对应直连 URL。
- `git diff --check` 通过；本次仅修改发布 workflow 和文档，未运行产品构建或 Tauri bundle。

## 验收

- 2026-08-09：用户验收通过；本次只提交并推送 workflow 与文档，不修改版本或创建发布 tag。

## 后续修订

- 2026-08-09：`v0.3.0` 首次使用该流程时发现，最终任务无法按 Tag 读取尚未公开的草稿 Release，`gh release download` 返回 HTTP 404。后续发布流程由 [`2026-08-09-08-updater-manifest-generation`](./2026-08-09-08-updater-manifest-generation.md) 取代，不再下载并规范化 `tauri-action` 的清单。
