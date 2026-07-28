Status: closed
Type: wayfinder:prototype
Parent: ../map.md
Blocked by: none
Assignee: codex

# 原型化紧凑托盘窗口

## Question

使用 `/prototype` 创建一次性 UI 原型，和用户共同决定概览与设置两层视图的信息架构、窗口尺寸、七日策略编辑、邮件配置、毛玻璃层次、深浅色、空状态、错误状态和通知入口。原型只回答外观与交互问题，不承诺最终框架；把选中的方向与截图作为 linked asset 保存。

## Comments

- 2026-07-28：开始 UI 原型。采用一次性独立原型页（当前仓库尚无桌面端页面可承载），
  提供三种结构差异明显的紧凑窗口，通过 `?variant=` 与底部切换器比较；原型分支只
  保存探索代码，main 只接收最终 UI 决策与截图。
- 2026-07-28：选择 **B — Weekly Ledger**。一次性原型保存在
  `prototype/tray-window-ui` 分支的
  `1d2a0ed9ef9dda0d30f0bc952f8d3c138d19becb`；main 只保存
  [`UI 决策与截图`](../../../docs/research/tray-window-ui.md)。窗口固定为
  420×680，概览以当前七日窗口为主结构，设置分为额度、账号、通知三类，
  light/dark、待配置、预警与数据过期状态均已验证。
