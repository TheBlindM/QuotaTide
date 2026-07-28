# QuotaTide 本地化与可访问性基线

> 状态：v1 实施基线
>
> 日期：2026-07-28
>
> 目标平台：macOS Apple Silicon / Intel、Windows x64

本文记录 QuotaTide v1 已确认的本地化、格式化、键盘和辅助技术契约。规范依据
与平台 API 链接见
[`localization-accessibility-source-notes.md`](./localization-accessibility-source-notes.md)。

## 支持语言与覆盖面

v1 正式支持：

| Resource locale | 用户显示名 | 状态 |
|---|---|---|
| `zh-CN` | 简体中文 | 完整支持 |
| `en` | English | 完整支持，也是最终 fallback |

两种语言必须同时覆盖：

- 托盘概览与全部设置；
- 系统通知及其 action label；
- 邮件 subject、纯文本和 HTML body；
- 校验、权限、采集、网络与发送错误；
- QuotaTide 自有安装说明、许可说明、关于页和诊断导出标签。

平台提供的标准安装按钮可以使用平台资源，但 QuotaTide 自己提供的文字不得
依赖平台自动翻译。两种语言都不是“社区尽力翻译”；缺 key、退回开发 key、
中英混排或 production UI 出现 test pseudo-locale 均为 release blocker。

所有用户可见字符串与执行代码分离，使用稳定 message key 和带名称的参数。
禁止在 Rust、Preact component、notification adapter、mailer 或 installer
template 中散落硬编码文案。翻译资源必须有描述性注释，说明参数语义、语气和
可用空间。

## 界面语言解析

设置值是：

```text
system | zh-CN | en
```

默认 `system`。用户选择写入非秘密配置，修改后立即更新当前窗口且无需重启。
设置为 `system` 时：

1. 调用 Tauri OS plugin 获取 BCP 47 tag；
2. 用标准 locale API 规范化并 maximize tag；
3. maximized language/script 为 `zh-Hans`，或 region 为 `CN`/`SG` 的简体
   中文，选择 `zh-CN`；
4. language 为 `en`，选择 `en`；
5. `zh-Hant-*`、其他不支持语言、无效 tag 或 `null`，选择 `en`。

因此裸 `zh` 按 CLDR likely subtags 解析为 `zh-Hans-CN`；`zh-TW`、
`zh-HK` 和 `zh-Hant` 不会被静默映射为简体。locale parsing 失败是普通
fallback，不向用户显示内部异常。

应用监听系统 language/locale change。只有设置为 `system` 时才即时切换界面；
手动选择保持不变。

## 界面语言、格式区域与策略时区

三者不能合并：

| 概念 | 来源 | 作用 |
|---|---|---|
| 界面语言 | `system/zh-CN/en` 解析结果 | 文案语言 |
| 格式区域 | 操作系统 locale/region | 日期顺序、12/24 小时、小数与分组符号 |
| 策略时区 | 用户配置的 IANA timezone | “今天”、七日策略归属、事件和重置所在时区 |

v1 不提供格式区域设置。有效系统 locale 原样交给 `Intl`；无法读取时，中文
界面使用 `zh-CN`，英文界面使用 `en-US`。格式区域只能改变呈现，不能改变
instant、自然日边界、额度 epoch 或历史事实。

## 格式化契约

统一封装 formatter，禁止 component、邮件或 adapter 自己拼接：

- 百分比：最多一位小数，移除无意义的尾随 `.0`；
- 重置倒计时：精确到分钟，不显示跳动秒数；
- 小于一分钟：使用本地化“即将重置”，而不是 `0 分钟`；
- 重置时间：同时提供相对值与绝对值；
- 绝对值：instant 转换到策略时区，再按格式区域决定日期顺序和 12/24 小时制；
- 数字、百分比、日期、时间、relative time、list 和 plural 全部使用 `Intl`；
- 未知值使用本地化“暂无数据”，不能显示 `NaN`、`Invalid Date`、空 token
  或 ASCII `-` 代替可访问状态。

视觉文本示例不是 canonical wire format：

```text
3 小时后 · 7月29日 10:01
in 3 hours · 7/29/2026, 10:01 AM
```

屏幕阅读器名称使用完整、不含视觉分隔符歧义的句子，例如“距离重置还有
3 小时；重置时间为 7 月 29 日 10 点 01 分”。

## 提醒语言快照

界面语言变化后：

- 当前窗口立即更新；
- 之后创建的提醒事件使用新语言；
- 已进入 outbox 的系统通知和邮件保持事件创建时语言；
- retry 不得重新读取当前语言而改变同一个 delivery 的文案；
- 应用内历史由结构化事件按当前界面语言重新呈现。

outbox 保存：

```text
message_key
structured_args
interface_locale_snapshot
format_locale_snapshot
policy_timezone_snapshot
```

canonical reminder/outbox data 不保存整段不可维护文案。delivery worker 从
snapshot 渲染 subject/body/action label；同一 delivery 的纯文本与 HTML
邮件必须使用同一 snapshot。

## WCAG 与视觉门禁

WCAG 2.2 AA 是 v1 的设计、实现和测试门禁，但项目不对外声称获得第三方正式
认证。

- 普通文本对比度至少 `4.5:1`；
- 大文本、图标、控件边界与 focus indicator 至少 `3:1`；
- 颜色、渐变或位置不能成为状态的唯一表达；
- icon control 的 pointer hit area 至少 `44 × 44 CSS px`；
- 紧凑内联控件最低 32px，并满足目标间距；
- 200% 字体缩放允许 420×680 窗口内部滚动，但不得截断操作、覆盖焦点或丢失
  功能；
- light、dark、不透明 fallback 和高对比模式分别测量真实相邻颜色，不能只测
  design token 名义值。

图表与七日柱状趋势必须有等价文本结构，包含日期、用量、今日实际上限和状态；
不能要求屏幕阅读器从几何高度理解数据。

## 键盘、焦点与播报

### 打开与 deep link

- 从托盘打开窗口时，原生窗口获得焦点，DOM focus 移到当前页面标题；
- 标题可程序化聚焦但不进入日常 Tab 顺序；
- 打开后播报页面概要一次，首次 Tab 进入第一个操作控件；
- 点击系统通知进入时，聚焦对应的今日额度、重置雷达或错误区域；
- deep link 的目标只播报一次，不能和 refresh live region 重复播报。

### 键盘

- `Tab` / `Shift+Tab` 顺序与视觉顺序一致；
- 设置分类作为标准 tabs，方向键移动并同步 `aria-selected`；
- `Escape` 在设置中返回概览，在概览中关闭窗口；
- `Cmd/Ctrl+,` 打开设置；
- `Cmd/Ctrl+R` 手动刷新；
- 禁止 hover-only 功能和无修饰单字母快捷键；
- 文件选择、SMTP 测试、chip 删除、开关、完成/返回和错误恢复都必须可纯键盘
  完成。

### 语义

- 使用原生 HTML element 优先于重造 ARIA widget；
- icon-only button、表单、图表、进度与状态都有本地化 accessible name；
- name 包含可见 label，不能让 Voice Control/Narrator 搜索名与画面文字冲突；
- 普通刷新和非紧急状态使用 polite `role="status"`；
- 需要立即处理的错误才使用 assertive `role="alert"`；
- `alert` 不自动夺取焦点，也不要求用户关闭；
- loading、stale、warning、exceeded、disabled 与 validation error 均暴露 name、
  role、value/state。

## 辅助显示设置

系统设置变化必须在运行时原子应用，不要求重启，且不能改变 DOM 结构、当前
焦点、数据或页面位置。

### 减少透明度

- macOS 原生 adapter 读取并监听 Reduce Transparency；
- WebView media query 只能作为额外信号，不能代替原生检测；
- 开启后完全关闭 blur、vibrancy、Acrylic 和 Mica；
- 使用不透明 surface token，不是降低 blur 或叠一层半透明白色。

### 高对比与 forced colors

- Windows adapter 监听系统 high contrast；
- WebView 使用 `forced-colors: active`、system color keywords 和可见边框；
- 停用品牌渐变、半透明与彩色阴影；
- Windows tray 原子切换到对应黑/白单色资产，不创建第二个 tray；
- 至少验证 High Contrast Black 与 High Contrast White。

### 减少动态效果

- 禁用窗口位移、旋转、进度补间、数值 tween 和 spinner；
- 刷新使用静态本地化“刷新中”文本与 busy state；
- 状态变化立即呈现，不以动画延迟可操作性；
- 普通模式只允许已确认的 150–240ms 窗口、刷新和状态动效。

## Release 验收矩阵

自动化门禁：

1. `zh-CN` 与 `en` key 集合完全一致，无 orphan/missing key；
2. production bundle 不包含伪本地化资源或裸 message key；
3. locale cases：`zh`、`zh-CN`、`zh-SG`、`zh-Hans`、`zh-TW`、
   `zh-Hant`、`en`、`en-GB`、unsupported、invalid、`null`；
4. percent/date/time/relative/plural snapshot 覆盖两种语言、两种 region 和
   DST 边界；
5. outbox retry 在切换语言后保持 locale/timezone snapshot；
6. axe 或等价自动检查无 critical/serious violation；
7. keyboard integration test 覆盖 overview、三类设置、保存、错误恢复和关闭；
8. pseudo-localization 将文案扩展约 40%，420px 与 200% 字体缩放不丢功能。

macOS 真机门禁：

- `zh-CN` / `en`、light / dark；
- VoiceOver 仅键盘完成查看额度、手动刷新、选择 `auth.json`、修改策略、保存
  通知设置和发送测试邮件；
- Increase Contrast、Reduce Transparency、Reduce Motion 分别及组合测试；
- 100% / 200% 字体缩放。

Windows 真机门禁：

- `zh-CN` / `en`、light / dark；
- Narrator 在关闭显示器情况下完成与 macOS 相同核心任务；
- High Contrast Black / White、forced colors、reduced motion；
- 100% / 200% 字体缩放。

系统通知、邮件、安装说明与关于页均在两种语言下人工检查。任何核心任务需要
鼠标、依赖颜色、读屏遗漏名称/状态、200% 下无法到达操作，或辅助显示模式仍
出现透明不可读背景，都阻塞 release。
