Status: open
Type: wayfinder:grilling
Parent: ../map.md
Blocked by: ./04-prototype-tray-window.md, ./07-decide-config-state-security.md, ./09-decide-product-identity-and-repository.md
Assignee: codex

# 决定本地化与可访问性范围

## Question

决定 v1 支持哪些界面、通知、邮件和安装文案语言，如何检测系统 locale 与回退，
以及键盘导航、屏幕阅读器、焦点、对比度、减少动态效果、减少透明度和高对比度
模式的验收边界。结论应覆盖 macOS 与 Windows，不能让毛玻璃视觉牺牲可读性。

## Comments

- 2026-07-28：开始确认 QuotaTide v1 的语言覆盖与可访问性验收边界。该票为
  HITL grilling，所有产品取舍由用户逐项确认，不能由代理代答。
- 2026-07-28：平台事实与一手来源已整理到
  [`docs/research/localization-accessibility-source-notes.md`](../../../docs/research/localization-accessibility-source-notes.md)。
  已确认 Tauri locale 返回 BCP 47 或 `null`、locale 与 region 需要分开处理，
  以及 VoiceOver/Narrator、WCAG 2.2、减少动态、减少透明度和 Windows
  forced-colors 的可验证平台边界；语言集合与产品回退仍等待用户决定。
