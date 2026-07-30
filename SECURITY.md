# Security policy / 安全策略

## English

QuotaTide handles a local Codex authentication file, quota metadata, optional
recipient addresses, and an SMTP password stored in the operating-system
credential vault. Please do not include any token, `auth.json`, SMTP password,
database, diagnostic archive, or private updater key in a public issue.

Until the public repository is bound, report a vulnerability privately to the
maintainer who provided this source tree. After binding, use the repository's
GitHub **Private vulnerability reporting / Security advisory** form. Include
the affected version, operating system, reproduction steps, and impact using
redacted fixtures.

Supported security fixes target the latest published `0.x` version. A
compromised or faulty release is repaired by a higher patch release; published
tags and assets are not replaced. Updater private keys and passwords are never
accepted in pull requests.

## 简体中文

QuotaTide 会接触本地 Codex 认证文件、额度元数据、可选收件地址，以及保存在
操作系统凭证库中的 SMTP 密码。请勿在公开 issue 中附带任何令牌、`auth.json`、
SMTP 密码、数据库、诊断压缩包或 updater 私钥。

公开仓库绑定前，请通过向你提供本源码的维护者私下报告。绑定后，请使用仓库的
GitHub **Private vulnerability reporting / Security advisory** 表单。报告应使用
脱敏 fixture，并包含受影响版本、操作系统、复现步骤和影响。

安全修复面向最新发布的 `0.x` 版本。发布故障或泄露事件通过更高 patch 版本
前滚修复；已经发布的 tag 和资产不会被覆盖。项目不会通过 pull request 接收
updater 私钥或密码。
