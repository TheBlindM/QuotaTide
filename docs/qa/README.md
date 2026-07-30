# Release evidence / 发布证据

This directory defines the auditable evidence package for Ticket 27. Research,
unit tests, development screenshots, or a locally built `.app` do not replace
installation and assistive-technology smoke tests performed on the exact final
release candidate.

本目录定义 Ticket 27 的可审计证据包。研究资料、单元测试、开发态截图或本机
构建的 `.app` 不能替代使用同一批最终候选产物进行的安装与辅助技术 smoke。

Create a matrix after checking out the exact release commit:

```bash
mkdir release-evidence-0.1.0
node scripts/qa/create-evidence.mjs \
  release-evidence-0.1.0/release-evidence.json
```

The generator creates 401 explicit `BLOCKED` records: the 393 primary
platform/test pairs, four WebView2 variants, and four best-effort compatibility
records (macOS 14 on Apple Silicon and Intel are separate). A tester must
replace them with `PASS`, `FAIL`, or an explicitly
approved `N/A` and fill the executor, timestamp, exact OS/build, CPU, WebView2
version where applicable, relative evidence paths, and linked defect.
Compatibility failures are non-blocking but require a linked defect and never
expand the support claim. The generated `requiredEvidenceType` is normative:
an `AUTO` result cannot replace required `SMOKE`, `MANUAL`, `LIVE`, or
`SECURITY` evidence. Artifact entries require the exact filename and SHA-256.

The versioned platform baseline is maintained in `scripts/qa/matrix.mjs`.
As of 2026-07-30 it requires macOS 15.7.8, current macOS 26.6, macOS 14.8.8
compatibility, Windows 11 builds 26200.8875 (25H2), 28000.2525 (26H1), and
26100.8875 (24H2 compatibility), plus Windows 10 build 19045.7548
compatibility. Refresh these exact values from Apple and Microsoft before
starting a later release; stale evidence cannot silently substitute older
patches.

版本化平台基线位于 `scripts/qa/matrix.mjs`。截至 2026-07-30，阻断版本为
macOS 15.7.8、当前 macOS 26.6、Windows 11 25H2 build 26200.8875 与
26H1 build 28000.2525；扩大兼容记录使用 macOS 14.8.8（两种 CPU）、
Windows 11 24H2 build 26100.8875 和 Windows 10 22H2 build 19045.7548。
开始后续版本前必须按 [Apple security releases](https://support.apple.com/100100)
、[Windows 11 release information](https://learn.microsoft.com/windows/release-health/windows11-release-information)
与 [Windows 10 release information](https://learn.microsoft.com/windows/release-health/release-information)
更新精确基线，旧补丁不能冒充“最新”。

生成器会有意把所有必测记录标为 `BLOCKED`。执行者必须改为 `PASS`、`FAIL` 或
有明确批准理由的 `N/A`，并填写执行者、时间、OS/build、CPU、适用时的
WebView2 版本、证据路径与关联缺陷。artifact 清单必须记录精确文件名和
SHA-256。

The public-release gate is:

```bash
node scripts/qa/check-evidence.mjs \
  release-evidence-0.1.0/release-evidence.json \
  release-assets \
  release-evidence-0.1.0
```

It fails when the file is not the final candidate, its version/commit differs
from the checked-out source, the exact seven release assets are absent or their
SHA-256 differs from final bytes, a blocking pair is not `PASS`/approved `N/A`,
the required evidence type differs, the platform identity is malformed, or a
referenced evidence file is absent. Every evidence path must resolve inside the
package.

After the gate passes, package the directory without an extra top-level folder
and attach it to the existing draft:

```bash
tar -czf release-evidence-0.1.0.tar.gz -C release-evidence-0.1.0 .
gh release upload v0.1.0 release-evidence-0.1.0.tar.gz --clobber
gh workflow run publish.yml -f tag=v0.1.0
```

`publish.yml` is the only supported public-publish path. It checks out the
immutable tag, confirms the release is still a draft, safely extracts the
evidence package, runs the mandatory gate against the downloaded final bytes,
and only then clears the draft flag. Configure the `public-release` GitHub
Environment with required reviewers and do not publish through the GitHub UI.

只有 `finalCandidate=true`、七类发布资产齐全、平台身份和证据类型正确、引用的
证据文件真实存在、每个阻断项都是 `PASS` 或有批准理由的 `N/A` 时，受保护的
`publish.yml` 才能公开发布。扩大兼容项的 `FAIL` 不扩大支持声明，且必须关联
缺陷。仓库必须为 `public-release` Environment 配置人工审批，并禁止绕过
workflow 在 GitHub 页面直接发布。
