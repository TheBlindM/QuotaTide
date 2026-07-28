# 25 — 完成双语和可访问性门禁

**What to build:** 让 QuotaTide 的全部核心工作流在简体中文和英语中都完整，
并能通过键盘、VoiceOver、Narrator、200% 字体和系统辅助显示设置独立完成。

**Blocked by:** 19 — 实现每日策略与工作日结转；20 — 接入 Reset Radar；22 — 实现持久提醒与系统通知；23 — 实现安全邮件投递；24 — 完成本地数据恢复与隐私工具

**Status:** ready-for-agent

- [ ] `zh-CN` 与 `en` 覆盖 overview、全部设置、tray、通知、邮件、错误、恢复、
  diagnostics、installer guidance、about 和 privacy 文案，key 集合完全一致。
- [ ] 语言设置支持 system/zh-CN/en；系统 locale 按已确认 BCP 47 规则解析，
  Traditional Chinese 和 unsupported locale 回退到 English。
- [ ] 界面语言、格式区域与策略时区保持独立；数字、百分比、日期、时间、
  relative time、list 和 plural 通过统一 Intl formatter。
- [ ] 百分比最多一位小数且移除 `.0`；倒计时精确到分钟并同时显示策略时区
  绝对时间；未知值不显示 NaN、Invalid Date 或裸占位符。
- [ ] outbox 保存语言、格式区域和策略时区 snapshot；切换语言后 retry 的
  通知与邮件保持创建时文案，应用内历史按当前语言重绘。
- [ ] 普通文本、控件边界、图标、状态和 focus indicator 满足 WCAG 2.2 AA
  对比门禁，状态不只依赖颜色。
- [ ] icon control hit area 至少 44×44 CSS px；420px 窗口、+40%
  pseudo-locale 和 200% 字体下所有核心操作可滚动到达且焦点不被遮挡。
- [ ] Tab/Shift+Tab、tabs arrow keys、Escape、Cmd/Ctrl+,、Cmd/Ctrl+R 和
  所有文件、策略、recipient、SMTP、保存、恢复操作可纯键盘完成。
- [ ] 图表有等价文本，状态和错误使用合适的 polite/assertive announcement，
  通知 deep link 只播报一次且不抢夺无关焦点。
- [ ] Reduce Transparency、Increase Contrast、High Contrast Black/White、
  forced colors 和 Reduce Motion 可运行时组合切换，不改变数据、DOM 顺序
  或当前焦点。
- [ ] 自动 axe/等价检查无 critical/serious；macOS VoiceOver 和 Windows
  Narrator 在无鼠标/显示器场景完成相同核心任务并留存证据。
