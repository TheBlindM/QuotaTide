# 27 — 执行支持矩阵与发布 QA

**What to build:** 使用同一批最终 release candidate 产物执行 QuotaTide 的
正式支持矩阵，形成可审计证据包；任何必测格未通过时阻止公开发布。

**Blocked by:** 26 — 完成安装、更新与开源清理

**Status:** ready-for-agent

- [ ] release evidence 记录 version、commit、artifact SHA-256、OS/build、CPU、
  WebView2、test ID、结果、执行者、时间、证据路径和 linked defect。
- [ ] macOS 15 最新补丁分别在 Apple Silicon 和 Intel 安装同一 universal
  artifact，验证原生 slice、首次启动、升级、卸载和重装。
- [ ] 当前最新 macOS Apple Silicon 完成全量功能、视觉与 VoiceOver smoke。
- [ ] Windows 11 25H2 x64 完成全量功能、WebView2、视觉、Narrator 和资源
  smoke；Windows 11 26H1 x64 完成新硬件兼容 smoke。
- [ ] WebView2 覆盖已安装 Evergreen、干净环境 bootstrapper、bootstrapper
  失败和 runtime 更新后的首次启动。
- [ ] 全量 smoke 覆盖 tray/window、材质降级、通知、autostart、dialog、
  auth rotation、vault、SMTP、SQLite recovery、clear data 和 updater。
- [ ] `zh-CN`/`en`、light/dark、200% 字体、reduced effects、forced colors、
  VoiceOver/Narrator 与通知/邮件/安装文案均完成规定人工门禁。
- [ ] production UI gzip 不超过 100 KiB；隐藏稳定 5 分钟后平均 CPU 低于
  0.5%；应用与专属 WebView 内存目标不超过 180 MiB；五次冷启动均不超过
  2.5 秒。
- [ ] 24 小时受控运行只出现批准的 Codex/Radar 小时请求和 updater 日请求，
  无隐藏轮询、线程增长或日志超过 5 MiB。
- [ ] canary 扫描覆盖 Public DTO、日志、diagnostics、snapshot、安装包、
  workflow artifact 和网络参数，任何秘密或禁止字段出现即失败。
- [ ] 真实上一版本到候选版本更新、篡改拒绝、失败保留旧版和 higher-patch
  roll-forward drill 均有证据。
- [ ] macOS 14、Windows 10 22H2、Windows 11 24H2 只记录 best-effort 扩大
  兼容结果，不能因单次通过进入正式支持声明。
- [ ] 所有必测项最终状态为 PASS/N/A 且 N/A 有批准理由；FAIL、BLOCKED 或
  未填写都会阻止发布。
