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
- 2026-07-28：用户确认 v1 完整支持简体中文（`zh-CN`）和英语（`en`）。
  覆盖应用界面、系统通知、邮件正文、错误提示和 QuotaTide 自有安装说明；
  两种语言都属于正式支持范围，不以“社区翻译/尽力而为”降级。
- 2026-07-28：用户确认界面语言设置提供“跟随系统 / 简体中文 / English”，
  默认跟随系统；手动覆盖持久保存并立即生效，不要求重启。系统 locale
  `zh-CN`、`zh-SG` 与 `zh-Hans-*` 映射为 `zh-CN`，`en`/`en-*` 映射为
  `en`；`zh-Hant-*`、其他不支持语言或 locale 读取失败均回退 `en`，不能把
  繁体中文静默映射成简体中文。
- 2026-07-28：用户确认界面语言、格式区域与策略时区彼此独立。界面语言控制
  文案；格式区域始终跟随操作系统 region，v1 不增加单独设置；策略时区决定
  “今天”、七日策略归属与重置时间所在时区。格式区域只能改变日期/数字呈现，
  不能改变 instant、自然日或额度事实。
- 2026-07-28：用户确认额度与用量最多保留一位小数并移除无意义的尾随 `.0`；
  重置倒计时精确到分钟，不显示跳秒，不足一分钟显示“即将重置”。重置同时
  提供相对值和绝对值；instant 按策略时区转换，日期顺序和 12/24 小时制按
  格式区域呈现。百分比、日期、时间、相对时间和复数统一使用 `Intl`，不得
  手工拼接。
