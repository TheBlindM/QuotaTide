# 本地化与可访问性一手资料笔记

> 状态：Ticket「决定本地化与可访问性范围」的事实输入
>
> 日期：2026-07-28
>
> 边界：本文只记录平台事实与标准，不替用户决定 v1 的语言范围或产品回退策略。

## Locale 与格式化

### 系统 locale 输入

Tauri 2 的 `@tauri-apps/plugin-os` 提供 `locale()`，返回操作系统 locale 的
BCP 47 language tag；无法取得时返回 `null`。因此实现必须显式处理无 locale
的情况，不能把该调用当成永远成功。

来源：[Tauri OS plugin `locale()`](https://v2.tauri.app/reference/javascript/os/#locale)

Unicode LDML/CLDR 使用 BCP 47/Unicode locale identifier，并定义规范化、
likely subtags 与 locale matching。例如 `zh` 的 likely locale 是
`zh-Hans-CN`，而 `zh-TW` 对应 `zh-Hant-TW`。这意味着不能仅用字符串前缀
随意把所有 `zh-*` 都当成简体中文。

来源：

- [Unicode LDML](https://unicode.org/reports/tr35/)
- [Unicode locale identifiers and likely subtags](https://unicode.org/reports/tr35/tr35-info.html)

Windows 把用户首选显示语言列表与 region 分开管理，并用 BCP 47 language tag
表示语言。Windows 的 app runtime language 先在用户语言与应用支持语言之间
匹配；显示语言资源与日期/数字使用的区域变体可以不同，例如应用只有 `en-US`
资源时，`en-GB` 用户仍可得到英国格式。

来源：[Microsoft — Understand user profile languages and app manifest languages](https://learn.microsoft.com/en-us/windows/apps/design/globalizing/manage-language-and-region)

### 数据格式化

ECMA-402 为 JavaScript 定义 `Intl.DateTimeFormat`、`Intl.NumberFormat`、
`Intl.PluralRules`、`Intl.RelativeTimeFormat` 和 `Intl.Locale`。应用界面不应
手工拼接日期、时间、百分号、复数或“多久以前”等 locale-sensitive 文本。

来源：[ECMAScript Internationalization API](https://tc39.es/ecma402/)

Microsoft 同样要求使用 globalization API 格式化日期、时间和数字，并避免
对语言、区域、书写系统、排序与数值格式做硬编码假设。

来源：[Microsoft globalization and localization guidelines](https://learn.microsoft.com/en-us/windows/apps/design/globalizing/guidelines-and-checklist-for-globalizing-your-app)

### 资源分离与测试

可本地化字符串应与执行代码分离。Microsoft 建议使用 pseudo-localization，
将字符串扩展约 40% 来暴露截断和硬编码问题；默认语言资源也必须明确标注语言。

来源：[Microsoft — Make your app localizable](https://learn.microsoft.com/en-us/windows/apps/design/globalizing/prepare-your-app-for-localization)

## 对比度、尺寸与缩放

WCAG 2.2 AA 要求普通文本至少 `4.5:1`，大文本至少 `3:1`；文本放大到 200%
时不得丢失内容或功能；键盘焦点必须可见且不能被作者创建的内容完全遮挡。
非文本 UI 组件及状态边界应满足 `3:1`。

来源：[WCAG 2.2](https://www.w3.org/TR/WCAG22/)

WCAG 2.2 AA 的 pointer target 最低是 `24 × 24 CSS px`，或必须满足规定的
间距/等价入口例外。QuotaTide 已有的 44px icon hit area 是高于该最低线的
产品基线，不应在实现时退回到仅勉强满足 24px。

来源：[WCAG 2.2 Target Size (Minimum)](https://www.w3.org/WAI/WCAG22/Understanding/target-size-minimum.html)

Apple 的可访问性指南对不超过 17pt 的普通文字采用 `4.5:1`，18pt 或粗体采用
`3:1`，并要求同时检查 light/dark appearance。Apple 还建议在 Increase
Contrast、Bold Text 与 Reduce Transparency 同时开启时验证核心任务。

来源：

- [Apple HIG — Accessibility](https://developer.apple.com/design/human-interface-guidelines/accessibility/)
- [Apple sufficient contrast evaluation criteria](https://developer.apple.com/help/app-store-connect/manage-app-accessibility/sufficient-contrast-evaluation-criteria/)

## 减少动态、减少透明度与高对比

CSS Media Queries Level 5 定义：

- `prefers-reduced-motion`；
- `prefers-reduced-transparency`；
- `prefers-contrast`；
- `forced-colors`；
- `prefers-color-scheme`。

来源：[W3C Media Queries Level 5](https://www.w3.org/TR/mediaqueries-5/)

`prefers-reduced-transparency` 的标准存在不代表每个目标 WebView 都已实现。
QuotaTide 的原生 macOS adapter 仍需读取
`NSWorkspace.accessibilityDisplayShouldReduceTransparency`；值为 `true` 时
Apple 明确要求不要使用半透明背景，并通过 display-options change
notification 监听运行时变化。

来源：[Apple `accessibilityDisplayShouldReduceTransparency`](https://developer.apple.com/documentation/appkit/nsworkspace/accessibilitydisplayshouldreducetransparency)

Apple Accessibility Inspector 可切换 Increase Contrast、Reduce
Transparency 和 Reduce Motion，属于 macOS 真机验收工具；减少透明度时窗口
应转为不透明表面，而不是仅降低 blur 数值。

来源：[Apple — Testing system accessibility features](https://developer.apple.com/documentation/accessibility/testing-system-accessibility-features-in-your-app)

Windows 高对比模式要求应用尊重系统颜色，而不是继续强制自己的品牌 palette。
原生 adapter 可以读取 `HIGHCONTRAST` 状态；WebView 内容同时使用
`forced-colors: active` 和 system color keywords，且必须在高对比黑、白方案
下测试。

来源：

- [Microsoft HIGHCONTRAST](https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-highcontrasta)
- [Microsoft high-contrast mode](https://learn.microsoft.com/en-us/windows/compatibility/high-contrast-mode)
- [Microsoft Edge forced-colors emulation](https://learn.microsoft.com/en-us/microsoft-edge/devtools/whats-new/2022/02/devtools)

## 键盘与辅助技术

WAI-ARIA 要求可访问对象暴露可理解的 name、role、value/state。动态普通状态适合
`role="status"`；需要立即打断的错误才使用 `role="alert"`。`alert` 是
assertive live region，不要求获得键盘焦点，也不应强迫用户关闭。

来源：[WAI-ARIA 1.2](https://www.w3.org/TR/wai-aria/)

Apple 的 VoiceOver 验收标准要求用户无需视觉协助即可：

- 聚焦所有重要元素；
- 听到准确、简洁的 label、role 与状态；
- 激活全部核心操作；
- 完成应用的常见任务。

来源：[Apple VoiceOver evaluation criteria](https://developer.apple.com/help/app-store-connect/manage-app-accessibility/voiceover-evaluation-criteria)

Microsoft 的 Narrator 测试要求使用 Tab、方向键和 Narrator 导航检查每个控件的
名称、类型和状态，并在关闭显示器的情况下只用键盘与 Narrator 完成主要场景。
自动化 accessibility tree 检查不能替代该真机任务测试。

来源：[Microsoft accessibility testing](https://learn.microsoft.com/en-us/windows/apps/design/accessibility/accessibility-testing)

## 通知、邮件与安装文本

系统通知中的标题、正文、按钮和状态仍是应用提供的用户界面文本，不会由
操作系统自动翻译。Windows 官方通知文档把 localization 与 accessibility
列为 notification content 的明确组成部分。

来源：

- [Microsoft app notifications overview](https://learn.microsoft.com/en-us/windows/apps/develop/notifications/app-notifications/)
- [Microsoft app notification content](https://learn.microsoft.com/en-us/windows/apps/develop/notifications/app-notifications/app-notifications-content)

邮件正文不属于 macOS/Windows 自动本地化资源；使用哪种语言、是在事件生成时
固定还是发送时读取当前语言，都是必须由产品明确的决策。安装器由平台提供的
标准按钮可随平台资源本地化，但 QuotaTide 自有的许可、说明、错误和包元数据
仍需单独提供已支持语言。

## 仍需用户决定

这些事实不能回答以下产品问题：

1. v1 支持的语言集合和默认 fallback；
2. 用户手动选择语言后是否覆盖系统 locale，以及何时生效；
3. 通知和邮件使用事件创建时语言还是发送时当前语言；
4. 时间、数字是否跟随界面语言，还是独立跟随系统 region；
5. WCAG 2.2 AA 是否作为完整产品 gate，以及是否采用更强的 44px target；
6. 哪些动画允许在 reduced-motion 下保留；
7. reduced-transparency/high-contrast 下是否完全禁用毛玻璃；
8. VoiceOver/Narrator 的 v1 核心任务验收清单。
