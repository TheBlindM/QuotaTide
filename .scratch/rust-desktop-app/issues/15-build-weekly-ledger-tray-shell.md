# 15 — 打通 Weekly Ledger 托盘窗口

**What to build:** 交付一个可演示的 QuotaTide 托盘壳。用户可以通过 Tide Dial
托盘图标打开 420×680 Weekly Ledger 小窗口、查看静态完整状态、进入设置，
并体验两端的原生材质与安全视觉降级。

**Blocked by:** 14 — 搭建 Rust/Tauri 可运行骨架

**Status:** ready-for-agent

- [ ] 左键托盘图标显示/隐藏窗口，右键显示“打开、立即刷新、退出”的原生菜单，
  关闭请求和普通失焦只隐藏，显式退出才终止应用。
- [ ] 窗口固定 420×680、不可最大化/最小化/resize，按 tray rect、显示器边界
  和 scale factor 锚定并夹取，失败时回退到当前显示器顶部居中。
- [ ] macOS 使用 accessory lifecycle 隐藏 Dock，Windows 窗口不显示任务栏
  按钮；失败进入可见诊断状态。
- [ ] macOS 尝试 Popover/HudWindow 材质，Windows 尝试 Acrylic 并允许 Mica
  降级；任一失败立即使用合格的不透明 surface。
- [ ] light、dark、reduced transparency、reduced motion、high contrast 和
  forced colors 都有确定的静态演示状态，不依赖材质才能操作。
- [ ] Weekly Ledger 概览用静态 fixture 展示周额度、今日实际上限、完整七日
  趋势、来源健康、Radar 区域以及正常/预警/超额/过期/待配置状态。
- [ ] 设置入口与返回路径可用，Escape、Cmd/Ctrl+,、Cmd/Ctrl+R 和基本 Tab
  顺序符合已确认交互。
- [ ] 打开右键菜单、快速重复点击、不同屏幕边缘和睡眠唤醒不会创建第二窗口或
  让窗口永久留在屏幕外。
- [ ] UI component 测试覆盖所有静态状态、light/dark 与不透明 fallback，
  截图或视觉证据使用选定的 B — Weekly Ledger 方向。
