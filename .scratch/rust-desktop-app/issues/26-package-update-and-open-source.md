# 26 — 完成安装、更新与开源清理

**What to build:** 把完整 QuotaTide 制作为用户可安装、可验证、可向前更新的
macOS/Windows `0.x` 未签名预览版候选，同时移除旧 Node/Docker 运行方式并
补齐开源项目所需元数据和安全说明。

**Blocked by:** 24 — 完成本地数据恢复与隐私工具；25 — 完成双语和可访问性门禁

**Status:** ready-for-agent

- [ ] macOS 生成同时含 arm64/x86_64 slice、最低系统为 15.0 的 universal
  DMG 和 universal updater archive。
- [ ] Windows 生成 Windows 11 25H2+ x64 current-user NSIS，使用 Evergreen
  WebView2 `embedBootstrapper`，不提供 MSI、per-machine 或 fixed runtime。
- [ ] 版本、产品名、作者、identifier、图标、版权、MIT License、
  third-party notices 和 about metadata 在所有产物一致。
- [ ] updater 启动稳定 60 秒后检查，随后每 24 小时检查，支持手动检查和关闭
  自动检查；不预下载、静默安装或强制重启。
- [ ] manifest 使用 SemVer、HTTPS URL、三个 platform entry；两个 macOS key
  指向同一 universal archive，Tauri signature 验证不可关闭。
- [ ] 错 key、缺 signature、篡改 artifact、无效 URL、取消、断网、磁盘不足
  和安装失败都保留当前版本可启动并显示脱敏错误。
- [ ] updater public key 和 fingerprint 进入仓库；private key/password 不
  进入 Git、artifact、日志或 fork PR，恢复流程要求两份独立加密副本。
- [ ] CI 与 release workflow 隔离权限，发布先建 draft、验证最终 bytes、
  checksum、provenance 和 updater signature，再发布不可变 Release。
- [ ] 未签名预览文档准确说明 Gatekeeper/SmartScreen，不声称系统已验证，
  不提供关闭系统级保护的命令。
- [ ] README、SECURITY、CONTRIBUTING、隐私说明、安装/卸载/更新/校验说明和
  独立项目声明可在 `zh-CN`/`en` 中找到。
- [ ] 删除 Node server、浏览器页面、Docker、明文 SMTP/env-primary 路径、
  旧依赖和旧运行数据库；保留的行为 fixture 已由 Rust 测试覆盖。
- [ ] GitHub remote 与 updater endpoint 仍使用显式 build-failing placeholder，
  未确认真实仓库时不能产生可发布 production artifact。
