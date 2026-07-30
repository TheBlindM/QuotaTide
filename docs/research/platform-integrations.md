# macOS 与 Windows 平台集成研究

研究日期：2026-07-28  
范围：Tauri 2 托盘应用的窗口壳、毛玻璃界面、通知、开机启动、文件选择与系统凭证库。本文只采用 Tauri、Apple、Microsoft 和所选 Rust 依赖的官方资料。

## 结论

v1 的平台集成方案可行，但必须区分“API 已存在”和“视觉/生命周期已经在真机证明”：

- 托盘、左击显示窗口、右键菜单、失焦隐藏、macOS 隐藏 Dock、Windows 隐藏任务栏图标都有官方 API 支持。
- Windows 10/11 可使用 Acrylic，Windows 11 还可使用 Mica；失败时降级为不透明主题背景。
- macOS 可使用 `Popover`/`HudWindow` 原生材质，但 Tauri 的窗口效果要求透明窗口，而 macOS 透明 WebView 需要 `macos-private-api`。Tauri 明确警告该 private API 会阻止 Mac App Store 上架。因此 v1 不能同时默认承诺“完整毛玻璃”和“Mac App Store 兼容”。
- 系统通知使用窄原生 Rust 边界；开机启动和原生文件选择器使用 Tauri
  官方插件。全部系统能力只从 Rust 侧调用，不向 WebView 开放通用 guest 权限。
- SMTP 密码使用 Rust `keyring` 生态接入 macOS Keychain 与 Windows Credential Manager；普通配置只保存稳定的凭证引用，绝不回退到明文密码。

## 平台能力矩阵

| 能力 | macOS | Windows | v1 降级与失败行为 |
| --- | --- | --- | --- |
| 托盘图标 | Tauri tray | Tauri tray | 创建失败属于不可恢复的壳层错误；显示本地错误窗口后退出，不静默驻留 |
| 左击弹窗、右击菜单 | 关闭 `show_menu_on_left_click`，左击事件显示窗口 | 同左 | 无事件时保留右键菜单中的“打开”和“退出” |
| 托盘相对定位 | Positioner `tray-icon` feature + tray event | 同左 | 先按托盘事件矩形自行夹取到当前显示器；仍失败则放到当前显示器顶部居中 |
| 隐藏应用入口 | `ActivationPolicy::Accessory`，不显示 Dock 和应用菜单栏 | `skipTaskbar: true` | 设置失败时应用仍可运行，但必须在诊断状态中显示平台错误 |
| 失焦隐藏 | `WindowEvent::Focused(false)` 时 `hide()` | 同左 | 正在打开文件对话框或通知权限提示时暂时抑制自动隐藏 |
| 毛玻璃界面 | `Popover` 或 `HudWindow` + transparent window | Windows 10/11 Acrylic；Windows 11 可回退 Mica | 任一原生效果失败则使用不透明、对比度合格的语义背景 |
| 系统通知 | `UNUserNotificationCenter`，显式检查/申请权限、稳定 request ID 和 delegate activation | WinRT `ToastNotifier`/`ToastNotification`，稳定 Tag + Group；正式外观只由已安装应用验收 | 拒绝或发送失败时保留提醒事件，在窗口中显示；邮件若已启用仍独立发送 |
| 开机启动 | autostart plugin，`LaunchAgent` | autostart plugin | 启用失败时回滚开关并显示可操作错误，不影响手动启动 |
| 选择 `auth.json` | dialog plugin 的原生文件选择器 | 同左 | 取消不改变配置；不可读或结构无效则保留旧路径 |
| SMTP 密码 | macOS Keychain | Windows Credential Manager | 缺失、锁定或访问失败时禁用邮件发送并提示重新保存，不写明文 fallback |

## 1. 托盘与锚定小窗口

### 官方能力

Tauri tray API 同时提供 Rust 和 JavaScript 接口。托盘事件包含鼠标按钮、按下/释放状态和图标矩形；官方示例在左击释放时取得窗口并执行 `show()`、`set_focus()`。[Tauri System Tray](https://v2.tauri.app/learn/system-tray/)

托盘菜单默认会在左右键都弹出。调用 `show_menu_on_left_click(false)` 后可以采用本产品需要的交互：

- 左击：显示或隐藏紧凑窗口。
- 右击：打开只含“打开”“立即刷新”“退出”的原生菜单。

Tauri Positioner 支持 macOS 与 Windows。其 `tray-icon` feature 会记录托盘事件状态，用于 tray-relative positions；官方文档要求把每个 tray event 交给 `tauri_plugin_positioner::on_tray_event`。[Tauri Positioner](https://v2.tauri.app/plugin/positioner/)

### v1 决定

1. 窗口在启动时创建但 `visible: false`，固定尺寸、不可最大化、不可最小化、不可 resize。
2. 所有 tray/window 操作由 Rust 壳层负责；WebView 不持有窗口或托盘管理权限。
3. 左击释放时：
   - 把 tray event 交给 Positioner；
   - 使用 tray-relative position；
   - 再按 tray rect、窗口尺寸、目标显示器的 position/size 做边界夹取；
   - `show()` 后 `set_focus()`。
4. 再次左击可隐藏；窗口失焦时隐藏。Tauri 官方 `Builder::on_window_event` 文档提供了在 `Focused(false)` 时隐藏窗口的示例。[Tauri Builder](https://docs.rs/tauri/latest/tauri/struct.Builder.html)
5. 打开原生文件选择器、通知授权提示或系统凭证提示时设置 modal guard；guard 存在时，失焦不能立即隐藏窗口。
6. 关闭请求转换为 `hide()`，只有托盘菜单“退出”才结束进程。

### 必须由原型验证

- macOS 菜单栏在屏幕左/右侧、刘海、多个显示器和不同缩放比例下的位置。
- Windows 任务栏位于顶部、底部或侧边，以及不同 DPI/多显示器下的位置。
- 重复快速点击、右键菜单打开期间和应用从睡眠恢复后的 show/hide 状态机。

## 2. 隐藏 Dock 与任务栏图标

macOS 使用：

```rust
app.handle()
    .set_activation_policy(tauri::ActivationPolicy::Accessory)?;
```

Tauri 的 `set_activation_policy` 仅在 macOS 可用；默认是 `Regular`，官方示例支持改为 `Accessory`。[Tauri AppHandle](https://docs.rs/tauri/latest/x86_64-apple-darwin/tauri/struct.AppHandle.html)

Apple 对 `NSApplication.ActivationPolicy.accessory` 的定义是：应用不出现在 Dock，也没有应用菜单栏，但仍可以通过代码或点击窗口激活；它对应 `LSUIElement = 1`。[Apple ActivationPolicy accessory](https://developer.apple.com/documentation/appkit/nsapplication/activationpolicy-swift.enum/accessory)

Windows 在窗口配置中设置：

```json
{
  "visible": false,
  "skipTaskbar": true
}
```

Tauri 配置文档明确说明 `skipTaskbar` 会在 Windows 隐藏窗口任务栏图标；该能力在 macOS 不适用。[Tauri Configuration](https://v2.tauri.app/reference/config/) [Tauri Window](https://docs.rs/tauri/latest/tauri/window/struct.Window.html)

## 3. Apple 风格毛玻璃与 Windows 材质

### Tauri 能力边界

Tauri 的 `Effect` 包含：

- macOS：`Popover`、`Menu`、`Sidebar`、`HudWindow` 等语义材质；
- Windows 11：`Mica`、`MicaDark`、`MicaLight`；
- Windows 10/11：`Acrylic`；
- Windows 的 `Blur` 也存在，但不同版本有性能说明。

各效果的系统版本说明见 [Tauri Effect](https://docs.rs/tauri/latest/tauri/window/enum.Effect.html)。

`Window::set_effects` 明确要求窗口是 transparent。[Tauri Window::set_effects](https://docs.rs/tauri/latest/tauri/window/struct.Window.html#method.set_effects)

与此同时，Tauri 的 WebView API 明确说明：macOS transparent window 需要 `macos-private-api`，使用 private APIs 会导致应用无法被 Mac App Store 接受。[Tauri Window API](https://v2.tauri.app/reference/javascript/api/namespacewindow/)

因此，下列两句话不能同时作为 v1 保证：

1. 所有 macOS 构建都使用完整原生毛玻璃；
2. 同一构建可以进入 Mac App Store。

### v1 材质策略

macOS 独立分发构建：

- `transparent: true`
- `macOSPrivateApi: true`
- 首选 `Effect::Popover`，原型若层次或文字对比不合格则试 `HudWindow`
- `EffectState::FollowsWindowActiveState`
- CSS 根背景保持透明，内容卡片使用半透明语义层，不使用夸张的网页 glassmorphism

Windows：

- 首选 Acrylic，因为 Microsoft 建议把 background acrylic 用于 flyout、non-modal popup 和 light-dismiss pane 等短暂表面。[Microsoft Acrylic](https://learn.microsoft.com/en-us/windows/apps/design/style/acrylic)
- Windows 11 上若 Acrylic 出现兼容或性能问题，尝试 Mica。Microsoft 将 Mica 定义为适合长驻窗口的非透明动态材质，因此它是视觉降级，不是 Acrylic 的同义替换。[Microsoft Mica](https://learn.microsoft.com/en-us/windows/apps/design/style/mica)
- Windows 10 Acrylic 失败时直接使用不透明背景。
- 窗口固定尺寸且不允许拖动/resize，以规避 Tauri 文档中 Acrylic 在部分 Windows build 拖动/缩放时性能较差的已知路径。

所有平台的最终 fallback：

- 完全不透明的系统浅色/深色背景；
- 保留圆角、阴影、间距和信息层级；
- 文本与状态颜色达到无障碍对比度；
- 原生效果失败不能阻止额度采集、通知或设置保存。

### 尚未决定

Mac App Store 是否属于 v1 发布目标由“决定安装、更新与开源发布策略”处理。若它进入 v1，必须提供不启用 private API 的 store-safe 构建，并接受不透明视觉降级；不能尝试隐藏 private API 使用。

## 4. 系统通知

QuotaTide 使用一个很小的原生 Rust 边界，而不是把通用 notification plugin
能力暴露给 WebView：

- macOS 使用 `UNUserNotificationCenter` 获取真实授权状态、提交本地通知并接收
  被点击通知的 delegate callback。事件的 delivery key 映射为稳定 request
  identifier；Apple 文档说明相同 identifier 会替换原请求，重试前还会移除
  同 identifier 的已送达通知。[Apple notification request identifier](https://developer.apple.com/documentation/usernotifications/unnotificationrequest/identifier)
- Windows 使用 `Windows.UI.Notifications`。`ToastNotifier.Setting` 区分系统、
  用户或应用级阻止；稳定 `Tag + Group` 标识同一用户提醒；`Activated` 事件只把
  实际被点击通知的 target 送回现有托盘进程。`Failed` handler 随 toast 保持
  存活：同步失败直接返回 worker，较晚的异步失败按 delivery key 修正 SQLite
  状态并刷新应用内提醒，不用固定超时猜测投递成功。[ToastNotifier.Setting](https://learn.microsoft.com/en-us/uwp/api/windows.ui.notifications.toastnotifier.setting)、[ToastNotification](https://learn.microsoft.com/en-us/uwp/api/windows.ui.notifications.toastnotification)

Tauri shell 只持有窄的 `permission_state`、`request_permission`、`notify` 接口。
平台返回真实提交错误后，delivery 才会写成 delivered；平台错误原文不会进入
UI 或数据库。

### v1 决定

- 通知由 Rust 提醒调度器直接发送，不经过 WebView。
- 首次启动不立即申请通知权限；当用户完成初始配置或主动开启通知时申请，避免无上下文授权弹窗。
- 权限状态包含 `unknown`、`granted`、`denied`、`error`。
- `denied` 或发送失败不会吞掉提醒事件：
  - 事件仍写入本地数据库并参与去重；
  - 托盘窗口显示未送达状态和系统设置提示；
  - 已启用的邮件渠道仍独立尝试发送。
- 通知正文不显示 access token、完整邮箱、完整 Account ID 或 SMTP 错误原文。
- 同一 delivery 的稳定平台 ID 用于崩溃恢复后的替换，不叠加第二条用户提醒。
- 只有 notification activation callback 会唤起并聚焦对应区域；普通托盘打开
  不消费“最近发送”状态。
- 每次 activation 都携带单调递增的本进程序号，使连续点击同一区域的通知也会
  重新聚焦；设置页收到 activation 时先回到概览。
- `denied`/`error` 状态保留“重新检查权限”入口；该显式操作读取系统真实状态，
  不会在后台循环重新弹出权限请求。

### 真机验收

- macOS：首次申请、允许、拒绝、在系统设置中撤销后的状态。
- Windows：必须使用安装包安装后的应用验证名称、图标、通知投递和卸载/重装行为。
- 睡眠恢复、应用窗口已打开、应用隐藏和重复提醒去重场景。

## 5. 开机启动

Tauri autostart plugin 支持 macOS 与 Windows，提供 `enable`、`disable` 和 `is_enabled`；官方 Rust 示例在 macOS 使用 `MacosLauncher::LaunchAgent`。[Tauri Autostart](https://v2.tauri.app/plugin/autostart/)

v1 规则：

- 默认关闭，由用户在设置中明确开启。
- 开关从 WebView 调用本项目自己的窄 command；command 在 Rust 内部调用 autostart manager。
- 保存设置前先执行系统操作，再以 `is_enabled()` 回读确认。
- enable/disable 失败时不修改本地偏好，UI 显示错误和重试入口。
- 通过 autostart 启动时不显示窗口，只创建托盘、启动 Rust scheduler。

发布形式可能影响 macOS LaunchAgent 或 Windows 启动项表现，因此安装包和签名后的验证由发布链路 ticket 继续处理。

## 6. `auth.json` 原生文件选择

Tauri dialog plugin 提供 macOS 与 Windows 原生文件选择器，并明确返回文件系统路径。[Tauri Dialog](https://v2.tauri.app/plugin/dialog/)

v1 不向 WebView授予通用 dialog/filesystem 权限，而是只暴露：

```text
select_auth_file() -> Result<Option<AuthPathCandidate>, PlatformError>
```

Rust command 负责：

1. 打开单文件选择器，默认文件名提示为 `auth.json`，过滤 JSON。
2. 用户取消时返回 `None`，不改变配置。
3. 对返回路径做规范化并以只读方式打开。
4. 只解析所需字段，验证它确实是可用的 Codex `auth.json`。
5. 成功后才原子替换配置中的路径；失败则保留旧路径并返回脱敏错误。

路径可保存在普通配置中；文件内容、token 和原始解析错误不能进入 WebView、SQLite 或普通日志。

## 7. macOS Keychain 与 Windows Credential Manager

Apple 把 Keychain Services 定义为代表用户安全保存小块秘密数据的加密存储；SMTP 密码适合 generic password item。[Apple Keychain Services](https://developer.apple.com/documentation/security/keychain-services/) [Apple Generic Password](https://developer.apple.com/documentation/security/ksecclassgenericpassword)

Windows `CredWriteW` 会在当前登录令牌关联的用户凭证集中创建或更新凭证；`CredReadW` 从该用户凭证集读取。[Microsoft CredWriteW](https://learn.microsoft.com/en-us/windows/win32/api/wincred/nf-wincred-credwritew) [Microsoft CredReadW](https://learn.microsoft.com/en-us/windows/win32/api/wincred/nf-wincred-credreadw)

Rust `keyring` 4.1.5 默认 `v1` feature 已按 target 使用 `apple-native-keyring-store` 和 `windows-native-keyring-store`，并提供统一的 set/get/delete API；项目使用 MIT OR Apache-2.0。[keyring-rs README](https://github.com/open-source-cooperative/keyring-rs) [keyring Cargo.toml](https://raw.githubusercontent.com/open-source-cooperative/keyring-rs/main/Cargo.toml)

### v1 凭证模型

- service：永久应用标识 `dev.theblind.quotatide.smtp`
- user：由后续配置模型固定为 `sender-slot-a` / `sender-slot-b`
- secret：SMTP 密码或应用专用密码
- 普通配置只保存：
  - `credential_ref`
  - SMTP host/port/security mode
  - sender address
  - recipient addresses
- `credential_ref` 不是密码，也不能包含密码片段。

最终双 slot 更新与崩溃恢复语义见
[配置、状态与本地安全模型](./config-state-security.md)。

### 错误与降级

- `NoEntry`：显示“SMTP 密码未保存”，邮件渠道禁用。
- keychain/credential manager locked、denied 或系统错误：显示“系统凭证库不可用”，不自动删除引用。
- 保存新密码必须先写凭证库并回读确认，再提交普通配置。
- 删除邮件配置时尝试删除凭证；删除失败要显示可重试状态，不能假装已清除。
- 永远不提供明文文件、环境变量或 SQLite fallback。
- 日志只记录错误类别，不记录 service/user 之外的原始系统错误上下文，更不能记录 secret。

## 8. WebView 权限边界

虽然 Tauri 插件提供 JavaScript guest bindings，v1 不把以下通用权限授予 WebView：

- tray/window positioner；
- notification；
- dialog/filesystem；
- autostart；
- keyring；
- shell 或通用 HTTP。

WebView 只调用本项目定义的 DTO commands，例如：

- `get_dashboard`
- `get_settings`
- `update_quota_policy`（完整设置表单落地前的阶段性窄命令）
- `select_auth_file`
- `update_mail_settings`
- `set_notification_preference`
- `set_autostart`
- `refresh_now`

Rust commands 完成验证、授权和系统调用。这样即使 WebView 出现脚本问题，也不能直接读取任意文件、获取 SMTP 密码或发送任意系统通知。

## 9. 原型验收清单

文档研究已证明 API 路径存在，但下列证据必须由“原型化紧凑托盘窗口”和之后的双平台 CI/QA 提供：

### macOS

- Apple Silicon 真机上的 tray 左/右键、窗口锚定、失焦隐藏。
- 刘海、多显示器、不同缩放与全屏 Space。
- `Popover` 与 `HudWindow` 在浅色/深色、Reduce Transparency 开关下的截图。
- private API 独立构建的签名与公证可行性；不能据此推断 App Store 可行。
- 通知权限、Keychain 首次保存/读取/删除和拒绝访问。

### Windows

- Windows 11：Acrylic、Mica fallback、DPI 和顶部/侧边任务栏。
- Windows 10：Acrylic 和不透明 fallback。
- 安装后的通知名称/图标、任务栏隐藏、开机启动。
- Credential Manager 首次保存/读取/删除，以及无凭证和系统错误。

### 跨平台通过标准

- 任一视觉效果失败时，应用仍能显示、配置、采集和提醒。
- 窗口任何时候都不会完全落在可视屏幕外。
- 拒绝通知、开机启动或凭证权限不会让应用崩溃，也不会泄露秘密。
- WebView 无权直接执行上述平台操作。
