# 20 — 接入 Reset Radar

**What to build:** 在 Weekly Ledger 中增加独立的 Reset Radar 区域，展示当前
有效的 24 小时额外重置概率、说明和来源，同时保证第三方预测永远不会改写
当前账号的真实额度事实。

**Blocked by:** 18 — 建立当前七日额度账本

**Status:** closed

- [x] Radar adapter 只访问固定 `codex-resets.com` HTTPS origin，使用 10 秒
  超时并复用 client。
- [x] 解析并验证 `reset_chance_24h`、`observed_at`、`expires_at`、说明和
  来源 URL；概率越界、时间无效、过期或缺字段时不展示预测。
- [x] 概率按来源站档位语义向下取整展示，例如 75% 显示 `>70%`，不制造精确
  概率承诺。
- [x] UI 明确标注预测来自第三方、不是 OpenAI 承诺，并提供安全的原始来源
  链接。
- [x] Radar observation 与 source health 独立持久化；失败保留当前仍有效的
  last-known-good，并在过期后隐藏。
- [x] Codex 与 Radar 在同一刷新轮次并发，但任一失败都不撤销另一来源的成功。
- [x] Radar 公告或概率变化不能建立/关闭 quota epoch、改变周用量、每日上限
  或真实重置时间。
- [x] 新公告只触发下一轮账号重新核对；只有账号额度事实确认后才记录实际重置。
- [x] fixture contract tests 覆盖有效、过期、缺失、越界、非法时间、来源链接、
  bucket 边界与部分来源失败。
- [x] UI 测试覆盖有效预测、无预测、过期、来源失败和 Codex 成功/Radar 失败
  等组合状态。

## Verification

- Codex 与 Radar 网络读取并发，验证后的两种来源结果通过同一 SQLite
  transaction 发布一个自洽 `dashboard_revision`；上游任一失败只更新自己的
  health，SQLite 提交失败则整轮回滚。
- Radar 在未配置 Codex 账号时仍会启动并遵守一小时节奏；早于截止的 resume
  不会延后原 hourly deadline。
- Rust workspace tests、Clippy `-D warnings`、离线 advisory/license/source
  audit、UI lint/typecheck/tests/build、420×680 light/dark 视觉检查和 macOS
  `.app` release bundle 均通过。
- Spec 与 Standards 双轴复核均为 PASS。
