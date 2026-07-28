# macOS / Windows 发布链路一手资料笔记

> 状态：Ticket 06 调研输入，不是最终实施配置
>
> 日期：2026-07-28
>
> 范围：macOS 与 Windows 安装包签名、Tauri 2 updater、GitHub Actions /
> Releases，以及 v1 构建矩阵与回滚约束。只引用 Apple、Microsoft、Tauri、
> GitHub、Rust 项目等事实所有者的一手资料。

## 摘要

适合 v1 的发布形状是：

1. PR 只做无密钥的跨平台构建与测试。
2. `vX.Y.Z` 标签触发受保护的 `release` environment；签名 job 与发布 job
   分权。
3. macOS 在 macOS runner 生成一个同时支持 Apple Silicon 与 Intel 的
   universal `.dmg`，使用 Developer ID Application 签名、Apple 公证并
   staple。
4. Windows 在 Windows x64 runner 同时生成 NSIS `-setup.exe` 与 MSI；
   NSIS 是普通用户的主下载和 updater 载体，MSI 是企业部署补充。
5. 两个平台还要生成 Tauri updater bundle 与 `.sig`。这是独立于
   Developer ID / Authenticode 的第二层签名，不能省略。
6. 所有平台资产先进入 draft GitHub Release，验证完整性后一次发布；启用
   immutable releases 后，已发布 tag 与资产不再改写。
7. v1 的“回滚”采用向前修复：把上一版代码作为更高补丁版本重新构建、重新
   签名并发布。不要移动 tag、替换同版本资产，也不要在 v1 覆盖 Tauri 默认
   SemVer 比较器。

Windows 签名存在一个发布前必须解决的资格问题：Azure Artifact Signing
Public Trust 目前仅向美国、加拿大、欧盟、英国的组织，以及美国、加拿大的
个人开发者开放。若开源项目的签约实体不符合范围，就必须另购受信任 CA 的
Authenticode 证书或调整发布主体，不能把 Artifact Signing 当成已可用能力。
[Microsoft quickstart](https://learn.microsoft.com/en-us/azure/artifact-signing/quickstart)

## 1. macOS

### 1.1 账号、费用和证书

- 对浏览器下载、在 Mac App Store 之外分发的软件，Apple 提供
  `Developer ID Application` 证书；签名后再提交公证，Gatekeeper 才能验证
  软件未被篡改且不是已知恶意软件。
  [Apple Developer ID certificates](https://developer.apple.com/help/account/certificates/create-developer-id-certificates/)
- Apple Developer Program 是 **99 USD/会员年**，部分非营利、教育机构和
  政府实体可能免除费用。个人/独资账户以个人名义加入；组织账户需要合法
  实体及 D-U-N-S Number。
  [Apple membership comparison](https://developer.apple.com/support/compare-memberships/)
- 免费 Apple Account 只适合开发与测试；Tauri 官方说明免费账户不能公证，
  下载后的应用仍显示为未验证。
  [Tauri macOS signing prerequisites](https://v2.tauri.app/distribute/sign/macos/#prerequisites)
- Apple 官方当前限制每个团队最多创建五个 Developer ID Application 和五个
  Developer ID Installer 证书。Developer ID certificate 的创建主体是
  Account Holder；组织的 Admin 只有被授予 cloud-managed Developer ID
  certificate access 时才可用云托管方式。
  [Apple Developer ID certificates](https://developer.apple.com/help/account/certificates/create-developer-id-certificates/)
- 本项目分发 `.app` / `.dmg`，核心证书是 `Developer ID Application`。
  `Developer ID Installer` 是签名 flat installer package 的证书；只有以后
  发布 `.pkg` 时才需要。
  [Apple Developer ID certificate types](https://developer.apple.com/help/account/certificates/create-developer-id-certificates/)

### 1.2 公证

- Apple 的 notary service 会扫描恶意内容和签名问题；通过后产生 ticket，
  可用 `stapler` 附到分发物上，Gatekeeper 也能在线找到 ticket。它不是
  App Review。
  [Apple notarization overview](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution)
- `altool` 和 Xcode 13 及更早版本自 2023-11-01 起不能再上传公证；自动化应
  使用 Xcode 的 `notarytool` / `stapler`，或 Notary API。
  [Apple notarization overview](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution)
- Apple 要求使用 Developer ID 分发的新软件公证；Tauri 也明确把公证列为
  `Developer ID Application` 证书的必需步骤。
  [Apple notarization requirements](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution)
  · [Tauri notarization](https://v2.tauri.app/distribute/sign/macos/#notarization)
- 公证前的代码应启用 Hardened Runtime、secure timestamp，不携带
  `com.apple.security.get-task-allow=true`，并确保所有嵌套 executable code
  都有有效 Developer ID 签名。
  [Apple notarization requirements](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution)
- `tauri build` 默认等待公证并 staple；`--skip-stapling` 会使构建不等待。
  正式 release 不应使用该参数，因为 stapled ticket 能让 Gatekeeper 在离线
  环境验证公证。
  [Tauri CLI `--skip-stapling`](https://v2.tauri.app/reference/cli/#build)

### 1.3 CI 凭证

Tauri 官方支持两类公证认证：

- App Store Connect API key：
  `APPLE_API_ISSUER`、`APPLE_API_KEY`、`APPLE_API_KEY_PATH`。私钥只能下载
  一次；CI 需要在 job 中把 environment secret 写入临时文件，并在 job 结束
  前删除。
- Apple Account：
  `APPLE_ID`、app-specific `APPLE_PASSWORD`、`APPLE_TEAM_ID`。

来源：[Tauri notarization credentials](https://v2.tauri.app/distribute/sign/macos/#notarization)

CI 中必须使用 **Team API key**：Apple 明确说明 individual API key 不支持
`notarytool`。Team key 由 Account Holder/Admin 管理，作用域是整个团队而不是
单一 app，因此仍要用受保护 environment 将其暴露面限制到 release job。
[Apple App Store Connect API keys](https://developer.apple.com/documentation/appstoreconnectapi/creating-api-keys-for-app-store-connect-api)

代码签名证书需把含私钥的 `.p12` 导出、base64 后作为
`APPLE_CERTIFICATE`，密码作为 `APPLE_CERTIFICATE_PASSWORD`；runner 导入
临时 keychain。`APPLE_SIGNING_IDENTITY` 可显式指定 Developer ID identity。
[Tauri signing in CI](https://v2.tauri.app/distribute/sign/macos/#signing-in-cicd-platforms)

发布建议：

- 优先采用 App Store Connect API key 做公证，避免把个人 Apple Account
  app-specific password 作为长期自动化凭证。
- `.p12`、其密码、API private key 都放在 GitHub `release` environment
  secrets 中；PR、普通 push、测试 job 不引用该 environment。
- release 后执行 `codesign --verify --deep --strict`、`spctl --assess` 与
  `xcrun stapler validate`，失败则不发布 draft。验证命令属于实施时的 release
  gate；Apple 对公证 ticket 与 Gatekeeper 的语义见
  [Apple notarization overview](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution)。

## 2. Windows

### 2.1 安装包选择

- Tauri 2 原生生成两类 Windows 安装包：WiX Toolset v3 的 `.msi` 和 NSIS
  的 `-setup.exe`。MSI 只能在 Windows 上构建；Tauri 虽说明 NSIS 可跨编译，
  但明确建议只有本地 VM 或 CI 不可用时才使用该路径。
  [Tauri Windows Installer](https://v2.tauri.app/distribute/windows-installer/)
- 默认 NSIS 是 per-user 安装，不要求管理员权限，目标为
  `%LOCALAPPDATA%`；`perMachine` 才要求管理员权限。对托盘工具，v1 以默认
  per-user NSIS 作为主下载更合适，MSI 保留给企业软件分发。
  [Tauri NSIS install modes](https://v2.tauri.app/distribute/windows-installer/#install-modes)
- MSI 构建需要 Windows 的 VBSCRIPT optional feature。CI 要固定 Windows
  runner image 并验证该能力，不使用漂移的 `windows-latest`。
  [Tauri MSI VBSCRIPT prerequisite](https://v2.tauri.app/distribute/windows-installer/#building)

### 2.2 Authenticode 与 SmartScreen

- Windows 本身允许运行未签名应用，但浏览器下载时会触发 SmartScreen 风险
  提示；Microsoft Store 上架和降低下载警告都要求签名。
  [Tauri Windows signing overview](https://v2.tauri.app/distribute/sign/windows/)
- Microsoft 2026 年官方说明已经纠正了常见旧认知：**EV 证书不再自动绕过
  SmartScreen**。有效 OV/EV 证书仍可能在新文件/新发布者阶段显示“无法识别”
  提示，信誉由 publisher identity 与 file hash 两类信号逐步积累。未签名
  文件每个版本都从零开始；持续使用同一受信任发布者身份有助于继承发布者
  信誉。
  [Microsoft SmartScreen reputation](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation)
- 因此，Tauri 页面中“EV 立即获得 SmartScreen reputation”的段落已经落后于
  Microsoft 当前文档，发布决策必须以 Microsoft 的新说明为准。
  [Tauri Windows signing](https://v2.tauri.app/distribute/sign/windows/)
  · [Microsoft SmartScreen reputation](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation)
- Authenticode 要签主 `.exe` 以及最终 NSIS/MSI；签名后不得修改文件。使用
  RFC 3161 timestamp，使签名能证明其发生在证书有效期内。Tauri 可通过
  `certificateThumbprint` / `digestAlgorithm` / `timestampUrl` 使用本机
  certificate store，也可用 `signCommand` 接入外部服务。
  [Tauri certificate configuration](https://v2.tauri.app/distribute/sign/windows/#prepare-tauriconfjson-file)
  · [Tauri custom sign command](https://v2.tauri.app/distribute/sign/windows/#custom-sign-command)

### 2.3 Azure Artifact Signing（原 Trusted Signing）

- 该服务已更名为 **Azure Artifact Signing**，此前名为 Azure Code Signing /
  Azure Trusted Signing；Tauri 支持用自定义 `signCommand` 接入。
  [Tauri Azure Artifact Signing](https://v2.tauri.app/distribute/sign/windows/#azure-artifact-signing)
- Public Trust 用于公开分享的 Win32 应用，证书链进入 Microsoft Root
  Certificate Program；Private Trust 不受 Windows 默认信任，不能替代公开
  下载软件的 Authenticode 公共信任。
  [Microsoft Artifact Signing trust models](https://learn.microsoft.com/en-us/azure/artifact-signing/concept-trust-models)
- Basic SKU 当前为 **9.99 USD/月/账户，含 5,000 次签名/月**；Premium 为
  99.99 USD/月/账户，含 100,000 次。超额均为 0.005 USD/次。两者都支持
  Public Trust，但仍要完成 identity validation。
  [Microsoft Artifact Signing SKU](https://learn.microsoft.com/en-us/azure/artifact-signing/how-to-change-sku)
- Public Trust 当前地域/主体资格有限：组织限美国、加拿大、欧盟、英国；
  个人开发者限美国和加拿大。项目在确认发布法律主体之前，不能把它写成无条件
  可用的唯一实现。
  [Microsoft Artifact Signing quickstart](https://learn.microsoft.com/en-us/azure/artifact-signing/quickstart)
- Microsoft 官方 GitHub Action 只运行在 Windows runner，支持
  `windows-2022`、`windows-2025`，当前不支持 Windows ARM runner；需要
  `Artifact Signing Certificate Profile Signer` RBAC role。
  [Azure artifact-signing-action](https://github.com/Azure/artifact-signing-action)
- 该 Action 推荐用 GitHub OIDC / federated credentials，而非
  `AZURE_CLIENT_SECRET`。release Windows job 只授予 `id-token: write` 与
  `contents: read`，Azure federation 再限制到仓库、`release` environment
  和 release tag；`id-token: write` 只允许请求 OIDC token，本身不授予
  GitHub 内容写权限。
  [Azure artifact-signing-action](https://github.com/Azure/artifact-signing-action)
  · [GitHub OIDC permissions](https://docs.github.com/en/actions/reference/security/oidc#workflow-permissions-for-the-requesting-the-oidc-token)

若主体不符合 Artifact Signing Public Trust 资格，回退路径是采购受信任 CA
签发的 Authenticode code-signing certificate，并按签发机构要求使用硬件或
远程私钥。Tauri 的旧 `.pfx` 示例明确只适用于 2023-06-01 以前取得的 OV
证书；新 OV/EV 的存储和 CI 流程必须遵循证书颁发机构当前文档，不能照抄旧
`.pfx` secret 流程。
[Tauri OV certificate warning](https://v2.tauri.app/distribute/sign/windows/#ov-certificates)

## 3. Tauri 2 updater

### 3.1 第二层签名与产物

- updater 签名不可关闭。应用内嵌公钥，构建时以私钥签 updater bundle；丢失
  私钥后，已安装应用无法再接受新更新。它必须有独立离线备份，不能只存在于
  一个 GitHub secret。
  [Tauri updater signing](https://v2.tauri.app/plugin/updater/#signing-updates)
- 构建环境变量是 `TAURI_SIGNING_PRIVATE_KEY` 与可选的
  `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`；`.env` 文件不生效。
  `bundle.createUpdaterArtifacts: true` 生成 v2 产物。
  [Tauri updater building](https://v2.tauri.app/plugin/updater/#building)
- macOS updater 产物是 `.app.tar.gz` 和对应 `.sig`；Windows v2 可直接使用
  NSIS `-setup.exe` / MSI 与对应 `.sig`。`v1Compatible` 只为 Tauri v1 用户
  迁移，v3 会删除，不适合全新项目。
  [Tauri updater artifacts](https://v2.tauri.app/plugin/updater/#building)
- Windows 同时生成 NSIS 与 MSI 时，`tauri-action` 默认因历史原因偏好 MSI；
  本项目应设置 `updaterJsonPreferNsis: true`，使 `latest.json` 与主渠道的
  per-user NSIS 一致。
  [tauri-action inputs](https://github.com/tauri-apps/tauri-action#usage)

#### Windows 的签名顺序

顺序必须是：

```text
patch app binary for bundle type
  -> Authenticode sign app binary
  -> build NSIS/MSI
  -> Authenticode sign final installer
  -> Tauri sign exact final installer bytes and write .sig
```

这不是约定猜测，而是 Tauri 当前源码的实际控制流：

- bundler 对每个 Windows bundle type patch 后重新签主 binary；
- NSIS 在 `makensis` 完成后调用 `try_sign`，MSI 在 WiX `light` 完成后调用
  `try_sign`；
- `tauri-cli` 等 `bundle_project` 全部返回后，才把返回的 installer path
  交给 `sign_updaters` 产生 `.sig`。

来源：
[Tauri CLI bundle then updater-sign, commit `5a882ec`](https://github.com/tauri-apps/tauri/blob/5a882eccfda53a189ec076c79c4ad186f50db5ff/crates/tauri-cli/src/bundle.rs#L230-L232)
· [Tauri updater signs returned bundle paths](https://github.com/tauri-apps/tauri/blob/5a882eccfda53a189ec076c79c4ad186f50db5ff/crates/tauri-cli/src/bundle.rs#L308-L320)
· [NSIS final-installer signing](https://github.com/tauri-apps/tauri/blob/5a882eccfda53a189ec076c79c4ad186f50db5ff/crates/tauri-bundler/src/bundle/windows/nsis/mod.rs#L714-L723)
· [MSI final-installer signing](https://github.com/tauri-apps/tauri/blob/5a882eccfda53a189ec076c79c4ad186f50db5ff/crates/tauri-bundler/src/bundle/windows/msi/mod.rs#L907-L916)

因此 Azure/CA 远程签名必须通过 Tauri `signCommand` 进入 bundling 流程。若在
`tauri build` 已生成 `.sig` 后再对 `.exe` / `.msi` 做 Authenticode 或任何
字节修改，现有 updater `.sig` 立即失效；此时必须重签 updater，不能直接
上传旧 `.sig`。

Developer ID / Authenticode 证明“操作系统信任这个发布者”；Tauri updater
`.sig` 证明“这个更新由应用内置 updater key 的持有者发布”。两层都要验证，
不能互相替代。

### 3.2 endpoint 与 `latest.json`

- production endpoint 强制 HTTPS。endpoint 数组只有在前一个返回非 2xx 时
  才继续下一个；不能假设 malformed JSON、无效签名或超时会自动容灾到后续
  URL。
  [Tauri updater configuration](https://v2.tauri.app/plugin/updater/#tauri-configuration)
- v1 采用 GitHub Release 静态 endpoint：
  `https://github.com/<owner>/<repo>/releases/latest/download/latest.json`。
  `tauri-action` 能生成该文件。
  [Tauri static updater JSON](https://v2.tauri.app/plugin/updater/#static-json-file)
  · [tauri-action](https://github.com/tauri-apps/tauri-action)
- 静态 JSON 的 `version` 必须是 SemVer；每个 `platforms.<OS-ARCH>` 至少含
  `url` 与 **`.sig` 文件内容**，不是签名 URL。Tauri 会在比较版本前验证整个
  JSON，因此列出的每个平台都必须完整有效。
  [Tauri static JSON schema](https://v2.tauri.app/plugin/updater/#static-json-file)
- GitHub “latest release” 是非 draft、非 prerelease 的发布；因此
  `vX.Y.Z-rc.N` 必须标为 prerelease，不能污染 stable endpoint。
  [GitHub Releases API](https://docs.github.com/en/rest/releases/releases)

### 3.3 安装与回滚语义

- 正常流程是 `check` → 下载并验证签名 → install → relaunch；Windows 默认
  `installMode` 为 `passive`，有进度窗口且无需用户完成向导。
  [Tauri updater check/install](https://v2.tauri.app/plugin/updater/#checking-for-updates)
  · [Windows install mode](https://v2.tauri.app/plugin/updater/#installmode-on-windows)
- 官方唯一明确说明的“安装较低版本”路径，是 dynamic update server 配合自定义
  version comparator，覆盖内部版本比较。静态 `latest.json` + 默认 SemVer
  比较不是降级通道。
  [Tauri dynamic updater rollback note](https://v2.tauri.app/plugin/updater/#dynamic-update-server)
- 官方文档没有提供“新版本启动失败后自动恢复旧二进制”的健康检查协议；不能在
  spec 中声称 updater 有事务式自动回滚。v1 应采用：
  - 发现问题时先撤销/停止传播有问题的 `latest.json`（若尚未 immutable
    publish）；
  - 对已经发布或安装的版本，发布更高补丁版本，例如把 `1.4.1` 的代码作为
    `1.4.3` 重建、重新进行平台签名与 updater 签名；
  - 数据库 schema 迁移保持前向兼容或可在新版本内修复，不能依赖二进制降级。

## 4. GitHub Actions 与 Releases

### 4.1 权限和密钥隔离

- 默认在 workflow 顶层写 `permissions: contents: read`；只有最终 publish
  job 写 `contents: write`。Tauri 官方 release 示例确认创建 release 所需的是
  `contents: write`。
  [GitHub least-privilege `GITHUB_TOKEN`](https://docs.github.com/en/actions/tutorials/authenticate-with-github_token)
  · [Tauri GitHub pipeline](https://v2.tauri.app/distribute/pipelines/github/)
- 只有 Azure OIDC 签名 job 加 `id-token: write`。GitHub 明确说明该权限仅
  允许请求 OIDC JWT，并不授予资源写权限。
  [GitHub OIDC permission](https://docs.github.com/en/actions/reference/security/oidc#workflow-permissions-for-the-requesting-the-oidc-token)
- `release` environment 配置：
  - 仅允许 `v*` tags；
  - required reviewer，阻止自审；
  - Apple 签名、公证 secrets 和 Tauri updater private key 只存在于该
    environment；
  - environment 通过审批前，job 不能读取 environment secrets。
  [GitHub environments](https://docs.github.com/en/actions/reference/workflows-and-actions/deployments-and-environments)
- fork PR 使用 `pull_request`，只给 read-only token，不引用 release
  environment。GitHub 默认不把除 `GITHUB_TOKEN` 外的 secrets 交给 fork
  workflow；fork PR 的写权限通常还会降为 read。
  [GitHub secrets and forks](https://docs.github.com/en/actions/how-tos/write-workflows/choose-what-workflows-do/use-secrets#using-secrets-in-a-workflow)
  · [GitHub fork token permissions](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax#permissions)
- 不使用 `pull_request_target` checkout fork 代码。GitHub 明确警告该组合可能
  同时暴露 secrets / write token 并导致仓库接管。
  [GitHub secure use](https://docs.github.com/en/actions/reference/security/secure-use)
- 公共开源仓库不使用 self-hosted runner 执行 PR。GitHub-hosted runner 是
  临时干净 VM，而 self-hosted runner 可能被不可信 PR 持久化攻陷。
  [GitHub runner hardening](https://docs.github.com/en/actions/reference/security/secure-use#hardening-for-self-hosted-runners)
- 所有第三方 Action 固定到完整 commit SHA；GitHub 说明这是目前把 Action
  当作不可变版本使用的唯一方式。Dependabot/Renovate 通过受审 PR 更新 SHA。
  [GitHub action pinning](https://docs.github.com/en/actions/reference/security/secure-use#using-third-party-actions)

### 4.2 Draft、不可变发布与证明材料

- 每个平台先上传到 draft release，汇总后验证：
  - tag、`tauri.conf.json` 与安装包版本一致；
  - macOS 签名、公证、staple；
  - Windows Authenticode 与 timestamp；
  - updater `.sig` 对应每个 bundle；
  - `latest.json` 平台项完整，URL 指向同一 tag；
  - SHA-256 checksums。
- 验证通过后一次发布。GitHub 对 immutable releases 的官方推荐顺序正是
  “创建 draft → 附加全部资产 → publish”；发布后 tag 不能移动，资产不能
  修改或删除，并自动产生 release attestation。
  [GitHub immutable releases](https://docs.github.com/en/code-security/concepts/supply-chain-security/immutable-releases)
- 发布 job 可额外生成 GitHub artifact attestation；它提供构建 provenance，
  但不能替代 Developer ID、Authenticode 或 Tauri updater signature。
  [GitHub artifact attestations](https://docs.github.com/en/actions/concepts/security/artifact-attestations)
- 为避免多个 tag 并行写同一个 release，workflow 使用 release concurrency
  group；同一版本只允许一个 publish job。
  [GitHub deployment concurrency](https://docs.github.com/en/actions/how-tos/deploy/configure-and-manage-deployments/control-deployments)

## 5. v1 构建矩阵与版本建议

### 5.1 固定矩阵

| Job | 固定 runner | target / bundle | release 资产 |
|---|---|---|---|
| macOS | `macos-15`（ARM64 host） | `universal-apple-darwin`; `app,dmg` | signed + notarized `.dmg`; `.app.tar.gz`; `.sig` |
| Windows | `windows-2025`（x64） | `x86_64-pc-windows-msvc`; `nsis,msi` | signed `-setup.exe`; signed `.msi`; updater `.sig` |

- Tauri CLI 明确支持 `universal-apple-darwin`，但需要同时安装
  `aarch64-apple-darwin` 与 `x86_64-apple-darwin` Rust targets。
  [Tauri CLI build target](https://v2.tauri.app/reference/cli/#build)
- Rust 官方把 `x86_64-pc-windows-msvc` 和
  `aarch64-pc-windows-msvc` 都列为 Tier 1 with host tools；v1 先交付覆盖面
  更大的 x64。Windows ARM64 应作为后续独立矩阵扩展，不能宣称已支持；微软
  Artifact Signing Action 当前也不支持 Windows ARM runner。
  [Rust Windows targets](https://doc.rust-lang.org/stable/rustc/platform-support/windows-msvc.html)
  · [Azure artifact-signing-action](https://github.com/Azure/artifact-signing-action)
- 不使用 `*-latest` runner label。GitHub 官方说明这些标签会逐步迁移到新 OS，
  runner image 也按周更新；固定 OS label 并保留 lockfiles，减少不可重现
  变化。
  [GitHub runner images](https://github.com/actions/runner-images)

### 5.2 版本单一来源

- `src-tauri/tauri.conf.json > version` 是应用版本唯一来源；Tauri 官方也推荐
  该位置，未设置时才回退到 `src-tauri/Cargo.toml`。
  [Tauri versioning](https://v2.tauri.app/distribute/#versioning)
- tag 必须严格等于 `v${version}`；release job 先校验 tag、Tauri version、
  Rust package version 和前端 package version 一致再接触签名密钥。
- 使用 SemVer：
  - 正式：`X.Y.Z`
  - 候选：`X.Y.Z-rc.N`，GitHub prerelease
  - 紧急恢复：提高 patch，不复用旧 tag。
- `Cargo.lock` 与前端 lockfile 提交到仓库；Tauri 官方说明 lockfile 用于让
  不同机器使用一致依赖。
  [Tauri configuration files](https://v2.tauri.app/develop/configuration-files/#cargotoml)

### 5.3 发布/回滚流程

```text
PR (无 secrets)
  -> macOS/Windows lint + test + unsigned bundle smoke
  -> merge
  -> signed tag vX.Y.Z
  -> protected release environment approval
  -> macOS universal sign/notarize/staple
  -> Windows x64 Authenticode sign
  -> Tauri updater sign
  -> draft release + latest.json + checksums
  -> install/update smoke on clean macOS + Windows VMs
  -> publish immutable release
```

回滚分三级：

1. **publish 前**：删除 draft 或重新上传修正资产，不对用户可见。
2. **publish 后、尚未广泛安装**：immutable release 的 `latest.json` 和安装
   包也不能被替换或删除，不能再“撤回并覆盖”；发布公告并立即发布更高 patch。
3. **已安装且存在严重缺陷**：以更高 patch 打包上一稳定代码并附带兼容的
   state/schema 修复；updater 正常向前升级。

只有将来确实需要分批发布、强制降级或渠道定向时，才引入 dynamic update
server 和自定义 comparator。那是新的受信任发布服务，不属于 v1 的 GitHub
Releases 静态端点范围。

## 6. 发布前未决条件

- Apple Developer Program 的 Account Holder、Team ID 与 Developer ID
  Application 证书尚未提供。
- Windows 发布法律主体与所在地尚未确认，因此 Artifact Signing Public Trust
  的资格未证明。
- GitHub 仓库 owner/name 尚未固定，无法写最终 updater endpoint 与 OIDC
  federation subject。
- Tauri updater signing key 尚未生成；生成后必须验证离线备份和恢复演练，
  再把公钥固化进应用。
- macOS/Windows 的最低系统版本要在实现后通过干净 VM 实测决定；本调研没有
  用推测替代兼容性验证。
