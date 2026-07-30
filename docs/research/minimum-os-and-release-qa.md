# QuotaTide 最低系统版本与发布 QA 门禁

> 状态：v1 实施基线，尚未通过真实安装包验证
>
> 决策日期：2026-07-28
>
> 产品：QuotaTide (`dev.theblind.quotatide`)
>
> 关联：[安装与更新策略](./distribution-policy.md) ·
> [平台集成](./platform-integrations.md) ·
> [应用架构](./application-architecture.md) ·
> [配置与安全](./config-state-security.md) ·
> [本地化与可访问性](./localization-accessibility.md)

本文把“框架理论上能运行”“操作系统或 WebView 仍受上游维护”和
“QuotaTide 对用户作出的支持承诺”分开。研究结论只能锁定候选基线和验证方法；
在 Rust/Tauri 应用、安装包和 updater 产物出现前，任何平台都不得标为
“已通过”。

## 结论

v1 候选最低运行环境：

| 平台 | 最低版本 / CPU | 产物 | v1 承诺 |
|---|---|---|---|
| macOS Apple Silicon | macOS 15.0 Sequoia，arm64 | universal DMG 中的 arm64 slice | 正式支持 |
| macOS Intel | macOS 15.0 Sequoia，x86_64 | universal DMG 中的 x86_64 slice | 正式支持 |
| Windows 11 | 25H2 x64；26H1 做新硬件兼容 smoke | per-user NSIS `setup.exe` | 正式支持 |
| macOS 14 Sonoma | Apple Silicon / Intel | 同一 universal DMG | 扩大兼容 smoke，不承诺 |
| Windows 10 22H2 / Windows 11 24H2 | x64 | 同一 x64 NSIS | 扩大兼容 smoke，不承诺 |
| Windows ARM64 | — | — | v1 不支持 |
| macOS 13 及更早 | — | — | v1 不支持 |
| Windows 10 21H2 及更早、Windows 7/8 | — | — | v1 不支持 |

“扩大兼容 smoke”只表示团队在有设备和时间时记录结果，不表示每个 release
都阻塞于该平台，也不构成持续修复承诺。Windows 10 22H2 已在 2025-10-14
结束常规支持；即使 Microsoft 承诺 Edge/WebView2 至少更新到 2028-10，
WebView 仍更新也不等于操作系统仍受支持。QuotaTide 不检测、绕过或替用户
声明 ESU 资格。

### 为什么不是框架的理论下限

- Tauri 默认 macOS deployment target 是 10.13，并允许通过
  `bundle.macOS.minimumSystemVersion` 提高下限；这只说明打包器能力，不是
  QuotaTide 的维护承诺。
- 当前 Apple 安全更新列表在 2026-07-27 同时发布 macOS Tahoe 26.6、
  Sequoia 15.7.8 和 Sonoma 14.8.8。Apple 没有在这些资料中承诺固定支持
  年限；选择 Sequoia 避免让尚未发布的 v1 一开始就承诺当前最老维护代际，
  同时仍覆盖 Intel 与 Apple Silicon。
- Xcode 当前仍可生成比 macOS 15 更低的 deployment target；这同样不能证明
  WKWebView、窗口材质、Keychain、LaunchAgent 和 updater 的组合已经可用。
- WebView2 能在更早的 Windows 上运行，不代表已结束支持的 Windows 应成为
  产品下限。Windows 11 25H2 Home/Pro 的官方支持期到 2027-10-12，避免了
  Windows 10 已 EOL 和 Windows 11 24H2 即将于 2026-10-13 EOL 的基线。

主要一手依据：

- [Tauri macOS minimum system version](https://v2.tauri.app/distribute/macos-application-bundle/#minimum-system-version)
- [Apple security releases](https://support.apple.com/en-us/100100)
- [Apple macOS Sequoia compatible computers](https://support.apple.com/en-us/120282)
- [Apple Xcode SDK and system requirements](https://developer.apple.com/xcode/system-requirements/)
- [Microsoft Windows 11 release information](https://learn.microsoft.com/en-us/windows/release-health/windows11-release-information)
- [Microsoft Edge/WebView2 lifecycle](https://learn.microsoft.com/en-us/deployedge/microsoft-edge-support-lifecycle)
- [Microsoft Windows 10 ESU](https://learn.microsoft.com/en-us/windows/whats-new/extended-security-updates)

## 实施配置

实现必须显式固定下限，不能依赖打包器默认值：

```json
{
  "bundle": {
    "macOS": {
      "minimumSystemVersion": "15.0"
    },
    "windows": {
      "nsis": {
        "installMode": "currentUser"
      },
      "webviewInstallMode": {
        "type": "embedBootstrapper"
      }
    }
  }
}
```

构建 target：

```text
macOS:   universal-apple-darwin
          ├─ aarch64-apple-darwin
          └─ x86_64-apple-darwin
Windows: x86_64-pc-windows-msvc
```

具体 Tauri schema 名称在依赖锁定后由 config validation 确认；上面的值是必须
实现的行为，不允许因为字段变动而回退成默认 deployment target、per-machine
安装或 fixed WebView2 runtime。

依赖 floor 同样必须显式化：

- Tauri 官方插件当前最低 Rust 是 1.77.2，但 `keyring` 4.1.5 的 Windows
  native store 声明 Rust 1.88；首次 lockfile 暂按 **Rust 1.88+**，再用两端
  CI 求实际依赖交集并写入 `rust-toolchain.toml`；
- `tokio-rusqlite` 启用 `bundled`，不让系统 SQLite 版本形成隐藏 OS floor；
- `lettre` 使用 `tokio1-rustls`，锁定 crypto provider 与 root store 后才做
  最低平台 SMTP smoke；
- 实现前不杜撰 `minimumWebview2Version`。先审计实际使用的 Web API/WRY
  能力，在干净 W25-X 记录 runtime 并验证 bootstrapper，再决定是否设置。

详细一手资料与依赖边界见
[最低系统版本一手来源笔记](./minimum-os-source-notes.md)。

## 证据等级

每个 release gate 必须标记证据种类。只有文档或开发态截图不能替代安装包
smoke。

| 标记 | 证据 |
|---|---|
| `AUTO` | CI 中可重复的单元、集成、静态或合约测试 |
| `BUILD` | release artifact 的结构、架构、签名、hash、权限或 bundle 检查 |
| `SMOKE` | 在指定 OS/CPU 上安装最终产物后进行的真实运行测试 |
| `MANUAL` | 需要视觉、交互、语言或辅助技术判断的人工测试 |
| `LIVE` | 对真实但隔离的外部系统进行的受控测试 |
| `SECURITY` | secret canary、权限、诊断包或网络边界检查 |

通过状态只有 `PASS`、`FAIL`、`BLOCKED`、`N/A`。`TODO` 不是 release 状态。
证据记录至少包含：

```text
release/version
commit SHA
artifact filename + SHA-256
OS edition/version/build
CPU architecture
WebView2 runtime version（Windows）
测试项 ID、结果、时间、执行者
日志/截图/报告的相对路径
已知偏差与 issue 链接
```

所有 release 证据统一放入 CI artifact `release-evidence-<version>`；人工 smoke
结果使用仓库中的版本化模板记录，不能只存在聊天、口头确认或个人桌面截图中。

## 必测平台矩阵

### 每次 release 的阻断矩阵

| ID | 环境 | 角色 | 必须完成 |
|---|---|---|---|
| M15-A | macOS 15 最新补丁，Apple Silicon | 最低 OS/arm64 | 全量安装与功能 smoke、资源预算 |
| M15-I | macOS 15 最新补丁，Intel | 最低 OS/x86_64 | 全量安装与功能 smoke、资源预算 |
| MC-A | 当前最新稳定 macOS，Apple Silicon | 当前主环境 | 全量 smoke、视觉、VoiceOver |
| W25-X | Windows 11 25H2 最新补丁，x64 | 最低 OS/主环境 | 全量安装与功能 smoke、资源预算、视觉、Narrator |
| W26-X | Windows 11 26H1 最新补丁，x64 | 新硬件兼容 | 安装、启动、托盘、通知、凭证、更新 smoke |

扩大兼容矩阵每个 minor release 和变更 platform adapter 时执行：

| ID | 环境 | 目标 | 失败含义 |
|---|---|---|---|
| M14-C | macOS 14 最新补丁，Apple Silicon + Intel | 判断能否诚实标记 best-effort | 不阻断正式平台，但必须禁止或提示不兼容安装 |
| W10-C | Windows 10 22H2 build 19045 x64 | 观察 WebView2/NSIS 兼容 | 不阻断；不得宣传为受支持 |
| W24-C | Windows 11 24H2 x64 | 观察升级过渡 | 不阻断；只记录到其生命周期结束 |

26H1 是面向特定新硬件的分支，不取代 25H2 主验证环境。将来 Windows 当前
版本变化时，矩阵更新为：

- 最低版本保持一台；
- Microsoft 推荐的广泛部署版本保持一台；
- 与其内核不同的新设备版本保持一台兼容 smoke；
- 已结束支持且不再位于最低版本的中间版本可轮转抽测。

### WebView2 变体

Windows 至少覆盖：

1. 已安装最新 Evergreen WebView2；
2. 干净 VM 中缺少 WebView2，由 `embedBootstrapper` 获取并安装；
3. bootstrapper 离线或下载失败，安装器给出可操作错误且不留下伪成功安装；
4. WebView2 自动更新后的首次启动；
5. 禁止使用固定 runtime 掩盖 Evergreen 兼容问题。

第二、三项至少在 W25-X 跑一次 release candidate；W10-C 只作为扩大兼容
证据。正常 release 不需要人为删除用户的系统 WebView2。

## 发布 QA 总矩阵

下面每一行都是阻断项；`平台` 未特别说明时表示 M15-A、M15-I、MC-A、
W25-X、W26-X。自动化负责证明确定性规则，真实构建负责证明平台生命周期。

### A. CPU、产物与安装

| ID | 验证 | 平台 | 证据 | 通过标准 |
|---|---|---|---|---|
| PKG-01 | universal slices | macOS | BUILD | `lipo -archs` 同时含 `arm64 x86_64`，两个 slice 都能原生启动 |
| PKG-02 | Windows 架构 | Windows | BUILD | PE 为 x64；不误标 ARM64/x86 |
| PKG-03 | 最低系统声明 | macOS | BUILD | `LSMinimumSystemVersion = 15.0`；macOS 14 不得显示为正式支持 |
| PKG-04 | 安装与首次启动 | 全部 | SMOKE | 最终 DMG/NSIS 可安装，首次启动只创建一份实例和一个托盘图标 |
| PKG-05 | 覆盖升级 | 全部 | SMOKE | 从前一公开版本升级，配置、SQLite、凭证引用和开机启动保持正确 |
| PKG-06 | 卸载 | 全部 | SMOKE | binary、启动入口和应用托盘消失；按文档保留或清理用户数据，不碰 `auth.json` |
| PKG-07 | 重装 | 全部 | SMOKE | 重装能识别保留数据；旧 schema 不重复 migration |
| PKG-08 | 未签名提示 | 全部 | MANUAL | 预览版准确显示 Gatekeeper/SmartScreen 风险，不引导关闭全局保护 |
| PKG-09 | identity/assets | 全部 | BUILD | 名称、作者、identifier、版本、图标、许可和 third-party notices 一致 |

### B. 托盘、小窗口与毛玻璃

| ID | 验证 | 平台 | 证据 | 通过标准 |
|---|---|---|---|---|
| SHELL-01 | 单实例 | 全部 | SMOKE | 连续启动只保留一个后台进程逻辑实例；已有窗口被安全唤起 |
| SHELL-02 | 左/右键 | 全部 | SMOKE | 左键显示/隐藏；右键只显示本地化菜单；菜单中可打开、刷新、退出 |
| SHELL-03 | 锚定与夹取 | 全部 | SMOKE | 多显示器、不同缩放、屏幕边缘、顶部/侧边任务栏下窗口不越界 |
| SHELL-04 | lifecycle | 全部 | SMOKE | 失焦隐藏；关闭等于隐藏；只有“退出”结束；睡眠恢复无重复窗口 |
| SHELL-05 | modal guard | 全部 | SMOKE | 文件选择、通知授权或凭证提示不会因失焦被错误关闭 |
| SHELL-06 | shell visibility | 全部 | SMOKE | macOS 不显示 Dock；Windows 不显示任务栏按钮；失败有可见诊断 |
| FX-01 | 原生材质 | macOS | MANUAL | Popover/HudWindow 在 light/dark、active/inactive 下可读且无黑底闪烁 |
| FX-02 | Acrylic/Mica | Windows | MANUAL | W10 Acrylic；W11 Acrylic 及 Mica 降级均不影响文字与交互 |
| FX-03 | 透明度关闭 | 全部 | MANUAL | Reduce Transparency/high contrast/forced colors 完全关闭 blur 和透明层 |
| FX-04 | 效果失败 | 全部 | AUTO + SMOKE | 强制 adapter 失败时立即使用合格的不透明 surface，核心能力不受影响 |

### C. 通知、开机启动与文件选择

| ID | 验证 | 平台 | 证据 | 通过标准 |
|---|---|---|---|---|
| NOTIFY-01 | 权限状态 | 全部 | SMOKE | unknown/granted/denied/error 都有稳定状态；不在无上下文时抢先请求 |
| NOTIFY-02 | 安装后身份 | 全部 | SMOKE | 最终安装应用的名称、图标和正文正确；Windows 不使用 dev/PowerShell 身份 |
| NOTIFY-03 | 点击通知 | 全部 | SMOKE | 打开现有实例并聚焦正确区域；不重复启动 scheduler |
| NOTIFY-04 | 去重/失败 | 全部 | AUTO + SMOKE | 同一事件不重复；拒绝/失败仍保留应用内提醒，邮件渠道独立 |
| START-01 | 启用/回读 | 全部 | SMOKE | 开关写入并回读；失败回滚 UI 与普通配置 |
| START-02 | 登录启动 | 全部 | SMOKE | 登录后静默驻留托盘，不主动显示窗口；仅一个 scheduler |
| START-03 | 升级/卸载 | 全部 | SMOKE | 升级不复制启动项；关闭或卸载后无幽灵启动入口 |
| FILE-01 | 原生选择 | 全部 | SMOKE | 只选单个 JSON；取消不改设置；modal guard 正常 |
| FILE-02 | 只读契约 | 全部 | AUTO + SECURITY | 每轮重新只读打开；应用绝不写、chmod、移动或删除 `auth.json` |
| FILE-03 | 旋转/错误 | 全部 | LIVE + SECURITY | Codex 自动改写 token 后下轮读取新值；无效 JSON/权限错误被脱敏 |

### D. 凭证库与 SMTP

| ID | 验证 | 平台 | 证据 | 通过标准 |
|---|---|---|---|---|
| VAULT-01 | set/get/delete | 全部 | SMOKE | Keychain/Credential Manager 两个固定 slot 可写、回读、删除 |
| VAULT-02 | Keep/Set/Delete | 全部 | AUTO + SMOKE | 三种更新语义不混淆，UI/IPC/SQLite 不回显 secret |
| VAULT-03 | 两阶段提交 | 全部 | AUTO | 每个 journal crash point 收敛到完整旧状态或完整新状态 |
| VAULT-04 | 锁定/拒绝/缺失 | 全部 | SMOKE | 邮件暂停且可恢复；通知与事实记录继续；不创建明文 fallback |
| VAULT-05 | 清除数据 | 全部 | SMOKE + SECURITY | 删除两个 app-scoped slot；失败时停止并告知，不声称已完成 |
| SMTP-01 | TLS relay | 全部 | LIVE | rustls TLS relay 可发中英测试邮件，证书验证开启 |
| SMTP-02 | STARTTLS | 全部 | LIVE | STARTTLS 升级成功；服务端不支持时明确失败；禁止明文 SMTP |
| SMTP-03 | 输入与 recipients | 全部 | AUTO | host/port/mode/from/recipient 校验；多收件人逐渠道持久化结果 |
| SMTP-04 | timeout/retry | 全部 | AUTO + LIVE | 超时、4xx、5xx 分类正确；退避、lease 和幂等不重复创建告警 |
| SMTP-05 | secret hygiene | 全部 | SECURITY | 日志、IPC、诊断、邮件错误、crash report 不含密码、完整邮箱或 auth 数据 |

### E. SQLite、恢复与领域行为

| ID | 验证 | 平台 | 证据 | 通过标准 |
|---|---|---|---|---|
| DB-01 | 初始化/权限 | 全部 | AUTO + SMOKE | 空库建完整 schema；应用目录权限不安全时拒绝写入 |
| DB-02 | migration | 全部 | AUTO + SMOKE | 从每个受支持旧 schema 逐级迁移；checksum 固定；失败完全回滚 |
| DB-03 | 新 schema | 全部 | AUTO | 数据库版本高于 binary 时只读失败，不降级、不覆盖 |
| DB-04 | WAL/crash | 全部 | AUTO + SMOKE | 非正常结束后 WAL 正常恢复；事实、当前状态和 outbox 原子一致 |
| DB-05 | 损坏恢复 | 全部 | AUTO + SMOKE | `quick_check` 失败时隔离原库；从最近有效备份恢复并重新验证 |
| DB-06 | 全部备份坏 | 全部 | AUTO + SMOKE | 进入专用恢复 UI，不静默创建空库 |
| DB-07 | 备份轮换 | 全部 | AUTO | 新备份完整性通过后才删除旧备份；始终保留最后有效恢复点 |
| DB-08 | 清除本地数据 | 全部 | SMOKE + SECURITY | 二次确认后清库/WAL/备份/日志/autostart/vault；不碰 `auth.json` |
| CORE-01 | 当前七日窗口 | 全部 | AUTO | 只采用账户当前严格 604800 秒窗口，不展示滚动最近七天 |
| CORE-02 | 额度高水位 | 全部 | AUTO | 微小回退不改历史；只有确认新 epoch 才重置 |
| CORE-03 | 动态工作日 | 全部 | AUTO | 当日未用工作日额度只分给之后工作日；历史不改；七日总量不超 100% |
| CORE-04 | hourly/single-flight | 全部 | AUTO + SMOKE | 启动一次、每小时一次、漏 tick 跳过；手动 30 秒冷却；并发触发合并 |
| CORE-05 | 来源隔离 | 全部 | AUTO + LIVE | Codex/Radar 独立 last-known-good；Radar 不能建立 quota epoch |
| CORE-06 | 告警 outbox | 全部 | AUTO + SMOKE | 阈值跨越、去重、重试、语言快照和渠道隔离符合契约 |

### F. 更新、发布与安全边界

| ID | 验证 | 平台 | 证据 | 通过标准 |
|---|---|---|---|---|
| UPDATE-01 | 检查节奏 | 全部 | AUTO + SMOKE | 稳定 60 秒后检查；之后每 24 小时；睡眠恢复只补一次 |
| UPDATE-02 | manifest/platform | 全部 | AUTO | SemVer、HTTPS URL、三个 platform key、universal 共用 archive 规则正确 |
| UPDATE-03 | signature | 全部 | AUTO + LIVE | 错 key、篡改包、缺 signature 一律拒绝；不能配置关闭验证 |
| UPDATE-04 | 用户确认 | 全部 | SMOKE | 不预下载、不静默安装；用户确认后才安装并重启 |
| UPDATE-05 | 跨版本 | 全部 | SMOKE | 前一公开版更新到候选版；设置、数据库、凭证和启动项保持正确 |
| UPDATE-06 | 中断/失败 | 全部 | SMOKE | 断网、磁盘不足、取消、安装失败后旧版仍可启动 |
| UPDATE-07 | roll-forward | 全部 | BUILD | 已发布产物不可覆盖；坏版本以更高 patch 修复；不自动降级 |
| UPDATE-08 | 隐私 | 全部 | SECURITY | updater 请求不携带 token、account、邮箱、额度或设备 ID |
| SEC-01 | capability allowlist | 全部 | AUTO | WebView 无通用 fs/http/shell/dialog/notification/autostart/updater/keyring 权限 |
| SEC-02 | CSP/resources | 全部 | AUTO + BUILD | CSP 默认 `self`；无远程脚本、字体或动态执行入口 |
| SEC-03 | canary scan | 全部 | SECURITY | Public DTO、日志、diagnostics、test snapshot 和 artifact 不含禁止值 |
| SEC-04 | 网络清单 | 全部 | LIVE + SECURITY | 只出现 Codex、Radar、用户 SMTP、GitHub updater 四类授权 origin |

### G. 语言、可访问性与视觉

本节的详细交互契约以
[本地化与可访问性基线](./localization-accessibility.md) 为准；本矩阵把它纳入
release blocking evidence。

| ID | 验证 | 平台 | 证据 | 通过标准 |
|---|---|---|---|---|
| L10N-01 | 资源完整性 | 全部 | AUTO | `zh-CN`/`en` key 对称，无裸 key、伪本地化或中英混排 |
| L10N-02 | locale/format | 全部 | AUTO | BCP 47 fallback、Intl、DST、百分比、相对/绝对时间符合契约 |
| L10N-03 | 外围文案 | 全部 | MANUAL | tray、通知、邮件、安装说明、恢复 UI、about/diagnostics 均双语 |
| A11Y-01 | 自动检查 | 全部 | AUTO | axe 或等价检查无 critical/serious；对比与语义规则通过 |
| A11Y-02 | 键盘 | 全部 | AUTO + MANUAL | 不用鼠标完成核心任务；焦点可见、顺序稳定、Escape/快捷键正确 |
| A11Y-03 | 缩放 | 全部 | MANUAL | 420px 宽、伪本地化 +40%、200% 字体下操作不截断且可滚动到达 |
| A11Y-04 | VoiceOver | macOS | MANUAL | 关闭鼠标完成查看、刷新、选文件、策略、通知和测试邮件 |
| A11Y-05 | Narrator | Windows | MANUAL | 关闭显示器完成同一核心任务 |
| A11Y-06 | 辅助显示 | 全部 | MANUAL | Reduce Motion/Transparency、Increase Contrast、forced colors 可组合使用 |
| A11Y-07 | 图表替代 | 全部 | AUTO + MANUAL | 七日趋势有等价文本日期、用量、上限和状态，不靠几何或颜色 |

### H. 资源预算

| ID | 预算 | 测量 | 通过标准 |
|---|---|---|---|
| PERF-01 | UI gzip ≤ 100 KiB | CI 对 production UI bundle 逐文件和总量 gzip | 不含 source map、系统 WebView 与 Tauri runtime；超限阻断 |
| PERF-02 | 隐藏空闲 CPU < 0.5% | 最低 macOS/Windows release build，隐藏稳定 5 分钟后连续测 5 分钟 | app process group 平均值均低于 0.5% |
| PERF-03 | 空闲内存 ≤ 180 MiB | 同环境，合计 QuotaTide 与专属 WebView 进程 | 稳定窗口内峰值和中位数均记录，目标不超过 180 MiB |
| PERF-04 | 冷启动 ≤ 2.5 秒 | 重启 OS 后从进程创建到 tray 可点击事件，连续 5 次 | 报告中位数和最慢值；每次均不超过 2.5 秒 |
| PERF-05 | thread/runtime | release build 运行时采样 | 一个 Tauri Tokio runtime、一个 SQLite 连接线程；刷新不持续增线程 |
| PERF-06 | 网络频率 | 24 小时受控运行和请求计数 | 正常每小时各一次 Codex/Radar；更新按 24 小时；无隐藏轮询 |
| PERF-07 | 日志上限 | 制造足量可脱敏错误 | 最多 `5 × 1 MiB`；轮转后总量不超 5 MiB |

测量约束：

- 不连接 debugger，不使用 dev server、hot reload 或开发 WebView；
- 使用最终 release profile 和当次待发布 artifact；
- CPU/内存以整个 QuotaTide process tree 为边界，不能只量 Rust 父进程而遗漏
  WebView2/WKWebView 辅助进程；
- 每次记录系统补丁、CPU、内存、WebView2 版本、电源模式和是否冷启动；
- 资源门禁至少在 M15-A、M15-I、W25-X 各执行一次；当前主环境保存趋势但不
  替代最低环境；
- 达不到预算只能用带双平台实测数据的 ADR 修改数字，不能删除测试或只提高
  某一个平台的阈值。

## Release 执行顺序

每个 release candidate 按下列顺序推进：

1. 锁定 Cargo/npm 依赖与版本，生成 SBOM、许可和 provenance；
2. Linux/macOS/Windows CI 跑 `AUTO` 与 contract tests；
3. 构建 universal DMG、macOS updater archive、Windows x64 NSIS 和 updater
   signature；
4. 对最终字节执行 `BUILD`、SHA-256、架构、版本、identity 和 secret scan；
5. 在必测平台安装同一批最终候选产物，执行 `SMOKE`；
6. 执行 `LIVE` SMTP、Codex、Radar 和 updater 沙箱测试；
7. 执行双语、VoiceOver/Narrator、对比、缩放和材质 `MANUAL`；
8. 汇总证据；任何必测格不是 `PASS` 就不发布；
9. 发布不可变 GitHub Release 后，重新从公开 URL 下载并校验 hash、updater
   manifest 和安装启动；
10. 保留报告，并把下一版本的最低 OS/上游生命周期复核日期写入 release
    checklist。

## 实现阶段必须用真实构建确认的事项

下列结论不能由当前研究关闭：

1. 当前 Tauri、wry、tao、positioner、notification、autostart、dialog、
   updater、keyring、lettre、rusqlite 版本组合确实能以 macOS 15 和
   Windows 11 25H2 为 runtime floor；
2. universal app 两个 slice 都没有只存在于单一架构的动态库或资源；
3. macOS private API 透明窗口在未签名预览安装路径下的启动、毛玻璃、通知、
   Keychain 与 LaunchAgent 生命周期；
4. Windows 10 Acrylic、Windows 11 Acrylic/Mica 与不透明 fallback 的视觉及
   功耗表现；
5. Windows 通知在安装后使用 QuotaTide 正确 identity；
6. per-user NSIS、WebView2 bootstrapper、autostart 和 updater 在覆盖升级/
   卸载后的真实行为；
7. Keychain/Credential Manager 的拒绝、锁定、清理和两 slot 崩溃恢复；
8. SMTP TLS/STARTTLS 与目标服务商真实互操作；
9. SQLite WAL、备份、损坏恢复和旧版本升级在实际文件系统与权限模型下工作；
10. 资源预算、200% 缩放、VoiceOver、Narrator 和系统视觉辅助模式达到门禁。

因此本文件的完成状态是“测试设计已就绪”，不是“应用已经兼容”。上述任何一项
在最低环境失败时：

- 先修复或提供已有设计允许的安全降级；
- 仍失败时记录证据并修改最低版本/支持矩阵 ADR；
- 更新安装器硬限制、README、release notes 和 updater platform metadata；
- 禁止只在网页上改写“推荐版本”而让不支持系统继续安装。

## 支持范围复核策略

每次公开 release 和每年 7 月至少复核一次：

- Apple security releases 是否仍对最低 macOS 发布安全更新；
- 最低 macOS 是否仍有 Intel 与 Apple Silicon 的可用测试硬件；
- 当前 Xcode/Rust/Tauri 是否仍能生成该 deployment target；
- Microsoft 是否仍维护最低 Windows 与 WebView2；
- Windows 10 22H2 的 ESU/WebView2 终止日期是否变化，以便维护准确的
  best-effort 说明；
- 当前 Windows 11 的广泛部署版本与特殊硬件版本；
- 所有直接平台依赖的 minimum OS、MSRV 和重大 lifecycle 变更。

若上游停止维护最低系统，新版本默认提升下限；旧版本可以保留下载，但不再称
为受支持版本，也不能通过未验证 updater 把不兼容 binary 推送给旧系统。

即使扩大兼容 smoke 当前通过，Windows 10 也不能进入正式下载页的支持矩阵。
若未来决定降低系统下限，必须重新做产品与安全 ADR，不能仅凭 WebView2
仍能启动就修改安装器。

## 当前阻塞与交接

- GitHub owner/repository 尚未确定，因此生产 updater endpoint、Release
  Environment、provenance 和最终下载 smoke 仍由仓库绑定决策补齐。
- Rust/Tauri 工作区、自动化门禁、universal app/DMG 与 release workflow 已
  实施；本机预检见
  [`docs/qa/0.1.0-local-preflight.md`](../qa/0.1.0-local-preflight.md)。
- 最终 updater key、同批 macOS/Windows release candidate、必测真机/VM、
  WebView2、辅助技术、真实 SMTP、跨版本和 24 小时受控运行仍未完成。自动化
  预检不能把这些 `SMOKE`、`LIVE`、`MANUAL` 项改写为 PASS。
- Ticket 27 的 JSON gate 要求正确平台身份、规定证据等级和真实证据文件；
  全部阻断项必须为 PASS 或有批准理由的 N/A。受保护的 `publish.yml` 是唯一
  支持的公开发布路径；当前 401 条记录显式 BLOCKED，因此不得公开发布。
