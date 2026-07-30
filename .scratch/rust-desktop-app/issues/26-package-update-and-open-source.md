# 26 — 完成安装、更新与开源清理

**What to build:** 把完整 QuotaTide 制作为用户可安装、可验证、可向前更新的
macOS/Windows `0.x` 未签名预览版候选，同时移除旧 Node/Docker 运行方式并
补齐开源项目所需元数据和安全说明。

**Blocked by:** 24 — 完成本地数据恢复与隐私工具；25 — 完成双语和可访问性门禁

**Status:** closed

- [x] macOS 生成同时含 arm64/x86_64 slice、最低系统为 15.0 的 universal
  DMG 和 universal updater archive。
- [x] Windows 生成 Windows 11 25H2+ x64 current-user NSIS，使用 Evergreen
  WebView2 `embedBootstrapper`，不提供 MSI、per-machine 或 fixed runtime。
- [x] 版本、产品名、作者、identifier、图标、版权、MIT License、
  third-party notices 和 about metadata 在所有产物一致。
- [x] updater 启动稳定 60 秒后检查，随后每 24 小时检查，支持手动检查和关闭
  自动检查；不预下载、静默安装或强制重启。
- [x] manifest 使用 SemVer、HTTPS URL、三个 platform entry；两个 macOS key
  指向同一 universal archive，Tauri signature 验证不可关闭。
- [x] 错 key、缺 signature、篡改 artifact、无效 URL、取消、断网、磁盘不足
  和安装失败都保留当前版本可启动并显示脱敏错误。
- [x] updater public key 和 fingerprint 进入仓库；private key/password 不
  进入 Git、artifact、日志或 fork PR，恢复流程要求两份独立加密副本。
- [x] CI 与 release workflow 隔离权限，发布先建 draft、验证最终 bytes、
  checksum、provenance 和 updater signature，再发布不可变 Release。
- [x] 未签名预览文档准确说明 Gatekeeper/SmartScreen，不声称系统已验证，
  不提供关闭系统级保护的命令。
- [x] README、SECURITY、CONTRIBUTING、隐私说明、安装/卸载/更新/校验说明和
  独立项目声明可在 `zh-CN`/`en` 中找到。
- [x] 删除 Node server、浏览器页面、Docker、明文 SMTP/env-primary 路径、
  旧依赖和旧运行数据库；保留的行为 fixture 已由 Rust 测试覆盖。
- [x] GitHub remote 与 updater endpoint 仍使用显式 build-failing placeholder，
  未确认真实仓库时不能产生可发布 production artifact。

## Comments

2026-07-30：完成 SQLite v12 自动更新偏好、Rust-only Tauri updater、严格
60 秒/24 小时调度、手动检查与二次确认安装；并加入三平台静态 manifest、
最终字节 checksum、Tauri signature 正反向校验、provenance、权限隔离和
只创建 draft 的发布工作流。公开仓库与最终 updater key 尚未确认，因此
`check-release-identity.mjs` 会同时阻断 `__GITHUB_REPOSITORY__` 和私钥已销毁
的开发 key fingerprint，这是预期的 production gate。

同批实现补齐中英 README、安全/贡献/隐私/安装/更新/卸载/校验文档，并删除旧
Node server、浏览器页面、Docker、明文 SMTP 环境入口与旧运行库。开发机已
构建出 macOS 15.0 universal app，并验证 `arm64`/`x86_64` slices。初次 DMG
因 `hdiutil` 自动估算的 HFS+ image 过小而失败；显式 128 MiB source image
后已成功生成、校验、挂载并检查，release workflow 已采用该固定尺寸脚本。
最终签名产物与真实平台 smoke 继续由 Ticket 27 执行。
