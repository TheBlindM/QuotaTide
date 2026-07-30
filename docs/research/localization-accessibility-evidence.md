# QuotaTide 本地化与可访问性实施证据

> Ticket：25
>
> 日期：2026-07-30
>
> 修订：包含本文档的提交

本文记录 Ticket 25 的可重复实施证据。支持矩阵中的 VoiceOver、Narrator、
真实高对比模式和最终安装包人工门禁仍由 Ticket 27 使用同一批 release
candidate 产物执行；未执行的人工项不得据此标为通过。

## 自动化结果

| 门禁 | 命令 | 结果 |
|---|---|---|
| UI lint、类型、测试、构建 | `npm --prefix ui run check` | PASS；5 个测试文件、58 项测试 |
| axe | 包含在 UI 测试 | PASS；overview 与四个 settings panel 无 critical/serious |
| Rust lint | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| Rust 测试 | `cargo test --workspace --all-targets --all-features` | PASS；live account 网络测试按设计 ignored |
| 依赖策略 | `cargo deny check` | PASS；advisories、bans、licenses、sources 均为 ok |
| 产品身份 | `node scripts/check-desktop-versions.mjs`、`check-desktop-identity.mjs` | PASS |
| 发布身份 | `node scripts/check-release-identity.mjs` | 预期 BLOCKED；真实 GitHub 仓库尚未绑定 |
| macOS `.app` | `cargo tauri build --bundles app` | PASS |

UI 生产资源 gzip 合计约 30.6 KiB，低于 100 KiB 发布预算。

## 已验证契约

- `system`、`zh-CN`、`en` 作为持久设置保存；裸 `zh`、简体地区和
  `zh-Hans` 解析为简体中文，繁体与不支持语言回退到 English。
- 界面语言、格式区域和策略时区独立保存与应用。百分比最多一位小数，重置
  倒计时精确到分钟并同时提供策略时区绝对时间和读屏完整描述。
- 提醒事件保存界面语言、格式区域和策略时区快照；切换语言后，已排队系统
  通知与邮件重试保持创建时语言。
- 概览、设置、恢复、诊断、隐私、托盘菜单、系统通知和邮件均提供简体中文与
  English。翻译 API 要求调用点同时提供两种文案；核心 formatter 的资源 key
  集合另有对称测试。
- settings 使用标准 tabs 语义、roving focus 和方向键/Home/End；标题在打开
  页面时获得程序化焦点；Escape、Cmd/Ctrl+, 和 Cmd/Ctrl+R 保持可用。
- icon button 和主要表单控件最小命中区为 44 × 44 CSS px；七日图表保留完整
  table 语义、日期、用量、上限和状态。
- Reduce Transparency、Increase Contrast 或 forced colors 变化时，UI surface
  与原生 Popover/Acrylic/Mica 材质同步切换；Reduce Motion 禁止动画。

## 浏览器视觉检查

使用生产组件的本地 preview，在实际 420 × 680 裁切中检查：

| 场景 | 状态 |
|---|---|
| `zh-CN` 与 English overview | PASS |
| English account/privacy settings | PASS |
| 文案扩展约 40% 的 pseudo locale | PASS |
| 200% 字体模拟下 overview、四个 settings tab 与底部操作 | PASS |

200% 下摘要改为单列，settings tabs 改为 2 × 2；所有页面保持纵向滚动。七日
数据表在极窄宽度下使用局部横向滚动，不裁掉全局操作或焦点。

## Ticket 27 必须补齐的人工证据

- macOS 已生成新的 `.app`，但本轮真实窗口检查遇到锁屏，状态为
  `BLOCKED`，不是 `PASS`。
- VoiceOver 核心工作流、Increase Contrast / Reduce Transparency /
  Reduce Motion 组合和真实 200% 字体需要在解锁后的支持版本 macOS 执行。
- Narrator、High Contrast Black/White、forced colors 和 Windows 200%
  字体必须在 Windows 11 25H2 x64 执行。
- 两个平台的系统通知、邮件、安装说明和未签名提示必须使用最终候选产物人工
  检查。

这些项目属于发布证据，不由开发态 DOM、axe 或截图替代。
