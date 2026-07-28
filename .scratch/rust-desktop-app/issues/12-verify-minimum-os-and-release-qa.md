Status: open
Type: wayfinder:research
Parent: ../map.md
Blocked by: ./02-verify-platform-integrations.md, ./04-prototype-tray-window.md, ./07-decide-config-state-security.md, ./08-decide-distribution-policy.md, ./09-decide-product-identity-and-repository.md, ./10-design-app-icon-and-tray-assets.md, ./11-decide-localization-and-accessibility.md
Assignee: unassigned

# 验证最低系统版本与发布 QA 矩阵

## Question

对照 Tauri、WebView2、macOS/Windows 平台与所选依赖的一手资料，确定 v1
暂定最低 macOS/Windows 版本和发布 QA 矩阵，并明确哪些结论必须在实现阶段
用真实构建 smoke 才能最终确认。矩阵必须覆盖
CPU、安装/卸载、托盘/窗口、毛玻璃降级、通知、开机启动、文件选择、凭证库、
SMTP、SQLite 恢复、更新、语言/可访问性和资源预算，形成 build-ready release
gate 文档。

## Comments
