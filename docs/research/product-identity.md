# QuotaTide 产品身份与开源元数据

> 状态：v1 实施基线
>
> 日期：2026-07-28
>
> GitHub 仓库：首次公开发布前绑定，当前不得猜测

## 公开身份

| 字段 | 固定值 |
|---|---|
| 产品名 | `QuotaTide` |
| 中文描述性副标题 | `Codex 额度助手` |
| 作者 | `TheBlind` |
| 社区归属 | `QuotaTide contributors` |
| 永久应用标识 | `dev.theblind.quotatide` |
| 可执行文件 | `quotatide` |
| 应用数据目录显示名 | `QuotaTide` |
| Rust package prefix | `quotatide-*` |
| License | MIT |

QuotaTide 是独立社区项目。产品名、图标、安装包和关于页不得使用 OpenAI
logo、官方 Codex 图标或暗示官方关系的名称。

建议固定英文声明：

```text
QuotaTide is an independent, community-built project.
It is not affiliated with, endorsed by, or sponsored by OpenAI.
Codex and OpenAI are trademarks of their respective owner.
```

中文声明：

```text
QuotaTide 是独立社区开发的开源工具，不隶属于 OpenAI，也未获得其背书或赞助。
Codex 与 OpenAI 是其权利人的商标；本项目仅用其说明兼容对象。
```

声明出现在 README 首屏产品说明之后、关于页、下载/Release 页面和
`SECURITY.md`。它不需要挤占托盘概览主界面。

## 平台与构建元数据

### Tauri

```text
productName = "QuotaTide"
identifier  = "dev.theblind.quotatide"
mainBinaryName = "quotatide"
```

发布构建不得从 Git branch、仓库 owner 或环境变量动态生成 identifier。
`dev.theblind.quotatide` 同时作为以下持久身份的根：

- macOS `CFBundleIdentifier`；
- Windows AppUserModel/installer identity；
- autostart entry namespace；
- notification identity；
- Keychain/Credential Manager service；
- 应用数据、cache 和日志目录归属。

首次公开版本后改变 identifier 会被视为另一个应用，并可能丢失配置、凭证、
通知权限和自动更新连续性，因此禁止更改。

### Rust workspace

```text
crates/quotatide-core       package = "quotatide-core"
src-tauri                   package = "quotatide-desktop"
binary                      name = "quotatide"
```

内部 module 可以使用领域名，不强制全部带品牌前缀；对外 crate、binary、
installer 和诊断元数据必须一致。

### macOS

```text
CFBundleDisplayName = "QuotaTide"
CFBundleName        = "QuotaTide"
CFBundleIdentifier  = "dev.theblind.quotatide"
CFBundleExecutable  = "quotatide"
Copyright           = "Copyright © 2026 TheBlind and QuotaTide contributors"
```

DMG：

```text
QuotaTide_<version>_universal.dmg
```

应用 bundle 显示 `QuotaTide.app`。当前未签名预览版不设置虚假的 Developer
ID team/publisher 字段。

### Windows

```text
ProductName      = "QuotaTide"
FileDescription  = "QuotaTide — Codex quota assistant"
InternalName     = "quotatide"
OriginalFilename = "quotatide.exe"
CompanyName      = "TheBlind"
LegalCopyright   = "Copyright © 2026 TheBlind and QuotaTide contributors"
```

安装包：

```text
QuotaTide_<version>_x64-setup.exe
```

NSIS product/uninstall/start-menu/autostart 名称都使用 QuotaTide。当前没有
Authenticode publisher，不能把 `CompanyName` 描述为受 Windows 验证的
publisher。

## 系统凭证与本地目录

SMTP Keychain/Credential Manager service 固定为：

```text
dev.theblind.quotatide.smtp
```

user slots：

```text
sender-slot-a
sender-slot-b
```

应用目录由系统 API 结合永久 identifier/name 解析，不能继续使用旧 Node
原型的“额度”、`quota-monitor` 或 Docker volume 名。实现阶段不迁移原型
SQLite；首次 Rust app 启动建立全新 QuotaTide 数据目录。

诊断文件显示产品名与版本，但不输出完整本地用户名、路径、GitHub token 或
credential references。

## 版权、License 与 notices

MIT LICENSE 顶部版权行：

```text
Copyright (c) 2026 TheBlind and QuotaTide contributors
```

源文件不要求机械添加 license header；仓库根 `LICENSE` 是规范来源。
第三方依赖 license 通过自动生成的 notices/SBOM 交付，不能把第三方版权改写
为 QuotaTide。

关于页：

```text
QuotaTide <version>
Created by TheBlind with community contributors
Licensed under the MIT License
Independent community project; not affiliated with OpenAI
```

贡献者名单来自 Git history/CONTRIBUTORS，不由 hard-coded UI 人工维护。

## GitHub 与 updater placeholder

当前仓库没有 Git remote，用户暂未确认公开 `owner/repo`。因此源码使用显式
release placeholder：

```text
repository = "__GITHUB_REPOSITORY__"
updater_endpoint =
  "https://github.com/__GITHUB_REPOSITORY__/releases/latest/download/latest.json"
```

规则：

- 普通开发 build 可以禁用 updater 并显示 `Development build`；
- CI 和 release build 检测到 `__GITHUB_REPOSITORY__` 必须失败，不能产生指向
  不存在地址的安装包；
- 不得默认使用 `benteli/quota-tide`、作者名或本机 Git username；
- 绑定仓库后同时更新 Cargo/package metadata、README、SECURITY、about links、
  updater endpoint、release workflow Environment 和 provenance subject；
- Git remote 与代码配置必须指向同一个 public repository；
- 仓库迁移需要 bridge release 或 redirect 兼容计划，不能只修改新 binary。

最终 slug 建议为 `quota-tide`，但它仍属于待完成任务，不是当前决定。

## 安全联系

公开仓库创建后：

- 启用 GitHub Private Vulnerability Reporting；
- `SECURITY.md` 的首选渠道是 repository Security tab 的私密报告；
- 普通 bug、功能请求和公开讨论使用 GitHub Issues/Discussions；
- v1 不公开个人邮箱；
- README 不要求用户在 Issue 中粘贴 token、`auth.json`、SMTP 密码或未脱敏
  诊断数据。

`SECURITY.md` 必须说明支持版本、预计首次确认时限、安全数据禁贴清单和未签名
预览版边界。没有公开仓库前保留模板，但不能声称私密报告功能已经启用。

## 视觉资产输入

后续图标原型必须围绕以下身份设计：

- 品牌词是 QuotaTide，不把 “Codex” 作为图标文字；
- 可使用“潮汐、周期、七日节奏、额度水位”的抽象语义；
- 不复制 OpenAI 结形 logo、Codex 图标或 ChatGPT 配色；
- 应用图标与单色 tray glyph 属于同一视觉家族，但 tray icon 优先小尺寸辨识；
- 中文副标题是文案，不烘焙进图标。

## 必须验证的身份测试

1. `tauri.conf`、Cargo metadata、frontend metadata 和 installer 使用同一版本
   与产品名。
2. bundle identifier 精确为 `dev.theblind.quotatide`。
3. binary 精确为 `quotatide`，安装包文件名符合双平台约定。
4. keyring service 精确为 `dev.theblind.quotatide.smtp`，只有两个 sender
   slots。
5. production/release build 含 repository placeholder 时失败。
6. Development build 不访问 placeholder updater URL。
7. README/about/Release 均有独立项目声明且没有官方 logo。
8. LICENSE 版权行与 Windows/macOS metadata 一致。
9. repository 绑定前不生成误导性的 SECURITY 私密报告链接。
10. 搜索旧品牌/运行时名称时，发布源码与资产中不存在 `quota-monitor`、
    Docker service 名或中文目录“额度”的用户可见残留。

## 仍待发布前绑定

唯一未固定的远程身份是 GitHub `owner/repo`。它已拆为独立任务，不阻塞本地
应用、图标、本地化或测试实现，但阻塞：

- production updater endpoint；
- GitHub Release workflow；
- artifact provenance subject；
- Private Vulnerability Reporting；
- 首个公开 `0.1.0` Release。
