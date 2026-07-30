# 27 — 执行支持矩阵与发布 QA

**What to build:** 使用同一批最终 release candidate 产物执行 QuotaTide 的
正式支持矩阵，形成可审计证据包；任何必测格未通过时阻止公开发布。

**Blocked by:** 26 — 完成安装、更新与开源清理

**Status:** ready-for-human

- [x] release evidence 记录 version、commit、artifact SHA-256、OS/build、CPU、
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

## Comments

2026-07-30：自动化证据框架已完成。生成器为五个正式环境与
PKG/SHELL/FX/NOTIFY/START/FILE/VAULT/SMTP/DB/CORE/UPDATE/SEC/L10N/A11Y/
PERF 创建 393 个主记录，并加入四个 WebView2 变体和四个扩大兼容记录，共
401 条显式 `BLOCKED`（macOS 14 Apple Silicon/Intel 分开记录）。校验器要求
exact final candidate、七类 artifact、
正确平台身份/证据等级、真实存在的证据文件和完整审计字段；阻断项的 FAIL、
BLOCKED、缺失记录与无批准理由 N/A 均拒绝发布。

本机预检已验证 universal slices、macOS 15.0、固定尺寸 DMG 挂载、updater
archive/signature 与篡改拒绝、31,685-byte UI gzip、secret canary、完整
Rust/UI/依赖门禁和五分钟后台资源趋势。证据见
[`docs/qa/0.1.0-local-preflight.md`](../../../docs/qa/0.1.0-local-preflight.md)。

Ticket 仍不能关闭：GitHub 仓库、最终 updater key/恢复演练和同批最终候选包
尚无；本机是 macOS 15.3 arm64 且桌面锁定，也没有 Intel Mac、Windows 11
25H2/26H1、WebView2 变体、VoiceOver/Narrator、真实 SMTP、跨版本更新与
24 小时运行环境。剩余工作需要维护者绑定发布身份并在规定真机/VM 上执行，
故转为 `ready-for-human`，所有 BLOCKED 状态继续阻止公开发布。

公开发布只允许通过受保护的 `publish.yml`：它下载既有 draft 的最终 bytes
与 `release-evidence-<version>.tar.gz`，在 immutable tag 上执行证据门禁，
通过后才清除 draft 标记。仓库绑定后仍必须给 `public-release` Environment
配置 required reviewers，并禁止在 GitHub UI 手工绕过。
