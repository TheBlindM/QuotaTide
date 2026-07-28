Status: closed
Type: wayfinder:research
Parent: ../map.md
Blocked by: ./01-choose-rust-desktop-stack.md, ./02-verify-platform-integrations.md
Assignee: codex

# 调研跨平台发布与更新链路

## Question

基于已选栈的一手文档，调查 macOS 签名与公证、Windows 安装包与代码签名、GitHub Releases、自动更新、版本回滚、CI 构建和开源贡献者发布权限。区分开发期可免费完成的部分与需要证书或付费账号的部分，在 `docs/research/release-pipeline.md` 留下推荐方案。

## Comments

- 2026-07-28：开始对照 Apple、Microsoft、Tauri、GitHub Actions 和 GitHub
  Releases 一手文档收敛发布链路。重点验证证书/账号成本、双平台 CI 隔离、
  updater 签名密钥、回滚语义和最小发布权限。
- 2026-07-28：结论记录于
  [`docs/research/release-pipeline.md`](../../../docs/research/release-pipeline.md)，
  逐条出处记录于
  [`docs/research/release-pipeline-source-notes.md`](../../../docs/research/release-pipeline-source-notes.md)。
  推荐 macOS universal DMG + Developer ID/公证、Windows x64 per-user NSIS
  + 条件式 Authenticode 路线、GitHub protected release environments 与
  mandatory Tauri updater signing。已确认 macOS 正式分发需要 99 USD/年
  会员；Windows Artifact Signing 受法律主体地域限制，不符合时需要 OV
  certificate + HSM/cloud signer。静态 updater 不做降级，事故以更高 patch
  roll-forward。
