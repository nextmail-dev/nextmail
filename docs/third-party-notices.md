# 随应用分发的字体

## Roboto

- 用途：NextMail 拉丁、希腊和西里尔界面字形，400/500/700 字重。
- 来源：`@fontsource/roboto` 4.5.8，字体上游为 Google Roboto v30。
- 许可证：字体为 Apache License 2.0；Fontsource 包装代码为 MIT。
- 上游：<https://github.com/googlefonts/roboto-2>

## Droid Sans Fallback

- 用途：NextMail 简体中文及其他 CJK 界面字形回退。
- 来源：AOSP `DroidSansFallback.ttf`，项目通过 `@fontpkg/droid-sans-fallback` 1.0.0 锁定二进制资源。
- 许可证：Apache License 2.0。
- 上游：<https://android.googlesource.com/platform/frameworks/base/+/dba35c0/data/fonts/>

字体只作为未修改资源随应用分发，不单独销售。发布包必须保留 Apache License 2.0 和对应版权声明；若未来替换字体，必须先更新本文件和锁文件并重新审查许可证。
