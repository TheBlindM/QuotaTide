# 安装、更新与卸载

## macOS 15+

从项目 GitHub Release 下载 universal DMG，并在打开前完成校验。将 QuotaTide
拖入“应用程序”。此预览版没有 Developer ID，也未经过公证，因此 Gatekeeper
可能阻止首次启动。

只使用 Apple 提供的单应用例外：在 Finder 中按住 Control 点击 QuotaTide 并
选择“打开”，或在系统设置出现时使用只针对该应用的“仍要打开”。QuotaTide
不会要求你全局关闭 Gatekeeper。

卸载时先退出 QuotaTide，再把“应用程序”中的 QuotaTide 移到废纸篓。如需同时
删除设置、历史、提醒 outbox 和凭证库条目，请先使用“设置 → 隐私 → 清除本地
数据”。该操作不会删除或修改 `auth.json`。

## Windows 11 25H2+ x64

从项目 GitHub Release 下载 x64 `setup.exe` 并完成校验。NSIS 安装包只为
当前用户安装到 `%LOCALAPPDATA%`，不需要管理员权限；缺少 WebView2 时会使用
Evergreen WebView2 bootstrapper。

由于预览版没有 Authenticode 证书，SmartScreen 可能显示“未知发布者”。只有在
确认 GitHub 来源和 SHA-256 后才继续。QuotaTide 不会要求你全局关闭
SmartScreen、Smart App Control、杀毒软件或证书校验。

从“设置 → 应用 → 已安装的应用 → QuotaTide”卸载。若还需删除设置、历史与
凭证库条目，请先使用应用内的本地数据清理功能。`auth.json` 保持不变。

## 更新

QuotaTide 在启动稳定 60 秒后首次自动检查，此后最多每 24 小时检查一次。可以
关闭自动检查；“账号 → 关于与更新”中的手动检查仍可使用。

发现更新后会展示版本和说明。只有选择“安装并重启”并再次确认后才会下载与
安装。断网、URL、签名、磁盘写入或安装失败时，当前版本仍保留可启动。即使
目前没有操作系统发布者签名，Tauri updater 签名验证也始终强制启用。
