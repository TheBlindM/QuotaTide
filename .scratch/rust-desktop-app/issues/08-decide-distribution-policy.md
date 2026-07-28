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
- 2026-07-28：用户确认 Windows v1 只提供 x64 per-user NSIS `setup.exe`，
  默认不提权并安装到当前用户目录；同一安装包作为手动下载与 Tauri updater
  载体。MSI 和 per-machine 企业部署不属于 v1。
- 2026-07-28：用户确认默认启用 GitHub Releases static endpoint 更新检查：
  启动稳定 60 秒后检查，之后每 24 小时一次，并支持手动检查和关闭自动检查。
  只有远端 SemVer 更高且 Tauri signature 有效才提示；不静默下载、不强制
  安装，用户点击“安装并重启”后才下载、再次验签并安装。
- 2026-07-28：用户决定当前开源项目不购买 Apple Developer ID 或 Windows
  Authenticode 证书，v1 作为未签名预览版公开。Release 必须明确说明
  Gatekeeper/SmartScreen 未知发布者提示，不引导关闭全局安全保护。免费生成
  的 Tauri updater signature 仍是强制安全边界；平台签名 seam 保留，未来
  获得证书后可启用。
- 2026-07-28：用户确认不提供自动降级。已发布 tag、manifest 和 assets
  不覆盖、不移动；故障时从已知良好代码发布更高 patch 版本并重新生成 updater
  signature。旧安装包可保留供手动恢复，但 static updater 只向前升级。
- 2026-07-28：用户确认未签名预览版从 `0.1.0` 开始，以普通 GitHub Release
  发布并在标题/说明中明确标注 Preview；不勾选 GitHub prerelease，以保证
  `/releases/latest/` static updater endpoint 可用。满足稳定门槛后再发布
  `1.0.0`。
- 2026-07-28：用户确认 v1 采用单维护者发布例外：只有受保护 `main` 上 CI
  通过的版本提交能经手动 workflow 和 GitHub Environment 确认发布；贡献者
  PR 不接触 updater private key 或 Release 写权限，workflow、updater public
  key 和发布脚本为敏感路径。有第二位可信维护者后启用双人审批和禁止自审。
- 2026-07-28：用户确认 Tauri updater key 使用密码保护；公钥与 fingerprint
  提交仓库，private key/password 只进入受保护 GitHub Environment，并由
  维护者保存两份离线加密备份。私钥不进入 Git 或安装包，正常轮换必须先用
  旧 key 发布包含新公钥的 bridge release。
