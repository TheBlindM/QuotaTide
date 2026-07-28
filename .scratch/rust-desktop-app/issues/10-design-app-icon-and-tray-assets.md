Status: in_progress
Type: wayfinder:prototype
Parent: ../map.md
Blocked by: ./04-prototype-tray-window.md, ./09-decide-product-identity-and-repository.md
Assignee: codex

# 设计应用图标与托盘资产

## Question

在产品身份确定后，制作并比较适合 Apple 风格简约毛玻璃界面的应用图标方向，
由用户选择最终方案；同时定义 macOS template tray icon、Windows tray icon、
light/dark/high-contrast 状态和 Tauri 所需的跨平台尺寸/格式。资产必须原创、
可随 MIT 项目公开分发，并在小尺寸下可辨识。

## Comments

- 2026-07-28：开始三方向视觉原型。问题是“QuotaTide 的应用图标应以哪种
  视觉隐喻表达潮汐、七日节奏与额度水位，并能收敛成小尺寸单色 tray glyph”。
  原型只用于选择方向，不作为最终生产资产。
- 2026-07-28：三方向对比原型已保存到一次性分支
  `prototype/quotatide-icon-assets`，提交 `2498f82`。本地运行地址为
  `http://127.0.0.1:4328/?variant=A`；已验证 A/B/C 切换、桌面与 390px
  窄窗口布局、16px 真实尺寸预览，以及 light/dark/high-contrast 托盘状态，
  浏览器控制台无警告或错误。等待用户选择 A、B 或 C 后，再制作原创、
  可确定性导出的生产级矢量与跨平台资产。
