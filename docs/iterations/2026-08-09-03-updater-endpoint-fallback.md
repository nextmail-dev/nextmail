# Updater Geo 响应与主备清单

日期：2026-08-09

## 状态

已验收。

## 范围

- 适配 Geo API 新增 `ip`、`type` 字段后的响应格式，区域判断仍只信任 `country_code`。
- 为 updater 配置 GitHub 直连与 NextMail 反代两个清单地址；CN 优先反代、直连备用，其他地区与 Geo 失败时优先直连、反代备用。
- 自动递增 patch 版本并直接发布，用于验证从 `v0.2.2` 到新版本的在线更新链路。

## 非目标

- 不改变更新签名信任根、清单格式、下载确认流程或 Geo 请求超时。
- 不运行前端或 Rust 全量测试，不执行本地 Tauri bundle。

## 验证门禁

- 针对性测试覆盖新 Geo 响应和两种区域下的 endpoint 顺序。
- 执行 `cargo check --offline --locked`、`cargo fmt --all -- --check` 与 `git diff --check`。
- 校验应用版本、CHANGELOG、公开密钥及两个 updater endpoints 后提交、推送并创建发布 tag。

## 实施结果

- Geo 响应继续只反序列化 `country_code`，自动兼容新增的 `ip` 与 `type` 字段，不收集或持久化 IP。
- Updater 在 CN 环境按反代、直连排序，在其他地区及 Geo 失败时按直连、反代排序；检查与安装使用相同的主备顺序。
- `tauri.conf.json` 同时声明 GitHub `latest.json` 与反代 `latest-cn.json`，保留公开验证密钥和签名校验。
- 应用版本递增为 `0.2.3`，补充 CHANGELOG，并同步 manifest、README、项目记忆与 `X-Mailer` 测试。

## 验证

- Geo/endpoint 排序测试：2 项通过。
- Updater 配置反序列化测试：1 项通过。
- 版本化 `X-Mailer` 测试：1 项通过。
- `cargo check --offline --locked`、`cargo fmt --all -- --check`、`git diff --check`：通过。
- 发布元数据校验确认三个版本源均为 `0.2.3`、公钥非空、endpoint 数量为 2、CHANGELOG 条目存在。
- 按用户要求未运行前端或 Rust 全量测试，未执行本地 Tauri bundle。
