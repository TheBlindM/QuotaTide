Status: closed
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
- 2026-07-28：用户选择 A — Tide Dial（潮汐仪表）。最终生产资产采用
  “圆形额度仪表 + 上升潮水”；大尺寸保留七日刻度，16–64 px 应用图标与
  tray glyph 使用删除刻度和细高光的光学校正版。
- 2026-07-28：原创 SVG、生成器、Tauri PNG/ICNS/ICO、macOS template
  tray 和 Windows color/high-contrast tray 已提交于 `25b8ff3`，实施规范见
  [`docs/research/icon-assets.md`](../../../docs/research/icon-assets.md)。
  已验证 PNG 为 8-bit RGBA，ICO 层与顺序满足要求，ICNS 可还原完整 iconset，
  连续导出逐字节一致；`npm run check` 与 27 项 `npm test` 全部通过。双轴
  code review 的 Spec 轴无问题；Standards 轴唯一问题为关闭记录缺失，已在
  本次 resolution 中修复。
