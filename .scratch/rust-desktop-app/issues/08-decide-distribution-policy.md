Status: in_progress
Type: wayfinder:grilling
Parent: ../map.md
Blocked by: ./05-choose-application-architecture.md, ./06-research-release-pipeline.md
Assignee: codex

# 决定安装、更新与开源发布策略

## Question

根据发布链路研究和应用架构，决定 v1 实际提供哪些安装包、是否默认检查更新、如何验证更新签名、怎样处理无签名开发构建，以及维护者和贡献者的发布权限。已确定 MIT License 和默认无遥测。

## Comments

- 2026-07-28：发布研究已确认 Windows Artifact Signing Public Trust 只覆盖
  美国/加拿大/欧盟/英国组织及美国/加拿大个人；若最终法律主体为中国个人或
  组织，当前不能采用该路线，需在 OV CA/cloud-HSM 与符合条件的开源签名服务
  中正式选择并做端到端原型。策略票还需确认 macOS universal + Windows x64
  矩阵、NSIS/MSI 范围、自动检查但用户确认安装，以及 GitHub static updater
  的 roll-forward 规则。
- 2026-07-28：开始逐项确认 v1 支持矩阵、安装包、签名主体、更新体验和
  开源发布权限。事实以发布链路研究为基线，只向用户确认产品与成本取舍。
- 2026-07-28：用户确认 v1 支持矩阵为 macOS Apple Silicon + Intel（一个
  universal build）以及 Windows x64。Windows ARM64 不作为首版承诺，待
  签名链和真机测试可重复后再扩展。
