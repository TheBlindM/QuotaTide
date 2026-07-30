# Release evidence / 发布证据

This directory defines the auditable evidence package for Ticket 27. Research,
unit tests, development screenshots, or a locally built `.app` do not replace
installation and assistive-technology smoke tests performed on the exact final
release candidate.

本目录定义 Ticket 27 的可审计证据包。研究资料、单元测试、开发态截图或本机
构建的 `.app` 不能替代使用同一批最终候选产物进行的安装与辅助技术 smoke。

Create a matrix after checking out the exact release commit:

```bash
node scripts/qa/create-evidence.mjs release-evidence-0.1.0.json
```

The generator intentionally marks every required record `BLOCKED`. A tester
must replace records with `PASS`, `FAIL`, or an explicitly approved `N/A` and
fill the executor, timestamp, OS/build, CPU, WebView2 version where applicable,
relative evidence paths, and linked defect. Evidence types are `AUTO`, `BUILD`,
`SMOKE`, `MANUAL`, `LIVE`, and `SECURITY`; combine required kinds with ` + `.
Artifact inventory entries require the exact filename and SHA-256.

生成器会有意把所有必测记录标为 `BLOCKED`。执行者必须改为 `PASS`、`FAIL` 或
有明确批准理由的 `N/A`，并填写执行者、时间、OS/build、CPU、适用时的
WebView2 版本、证据路径与关联缺陷。artifact 清单必须记录精确文件名和
SHA-256。

The public-release gate is:

```bash
node scripts/qa/check-evidence.mjs \
  release-evidence-0.1.0.json \
  release-assets
```

It fails when the file is not the final candidate, its version/commit differs
from the checked-out source, the exact seven release assets are absent or their
SHA-256 differs from final bytes, any required environment/test pair is
missing, any result is `FAIL`/`BLOCKED`, or audit fields are incomplete. The
JSON and referenced screenshots/logs are uploaded together as
`release-evidence-<version>`.

只有 `finalCandidate=true`、七类发布资产齐全、每个必测环境/测试组合都有完整
审计字段，并且所有状态都是 `PASS` 或有批准理由的 `N/A` 时门禁才通过。
