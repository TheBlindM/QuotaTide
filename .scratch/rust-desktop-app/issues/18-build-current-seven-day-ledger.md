# 18 — 建立当前七日额度账本

**What to build:** 把真实周额度观测转换成可恢复的当前账号七日账本，让用户
看到本次额度窗口的七个自然日、每日新增使用和可信状态，而不是滚动最近七天。

**Blocked by:** 17 — 接入当前账号真实额度

**Status:** closed

- [x] `QuotaLedger` 使用整数 QuotaUnits 处理上游观测，只在 adapter 边界转换
  浮点数据。
- [x] quota epoch 由当前账号严格窗口事实确认；`reset_at` 文本漂移、Radar
  预测或单一异常样本不能建立新 epoch。
- [x] 同一 epoch 的 used 采用持久高水位；微小回退不产生负用量或改写历史。
- [x] 新增用量按策略时区自然日分配，跨日与 DST 行为有确定规则；新 epoch
  当天继续累计但保留重置前事实。
- [x] 第一个样本、未知日期和采集中断不会被当成 0% 使用量推导虚假历史。
- [x] 不可变 usage observations、epoch、stream state、daily ledger 与
  dashboard revision 在一致事务边界内持久化。
- [x] 概览严格按当前 epoch 起点到重置前一天展示完整七个日期；空日期保留
  空位，不混入前一 epoch 或“最近七天”数据。
- [x] 七日图表同时提供日期、用量、上限和状态的语义文本结构。
- [x] 重启可从不可变事实重建投影，结果与退出前一致；账号切换后旧流隐藏但
  再次切回可恢复。
- [x] 表驱动和属性测试覆盖首次样本、高水位、确认重置、跨日、DST、账号切换、
  事务回滚和事实重建。

## Comments

- Resolution: commits `c1f75cb` through `7aba94c` deliver the integer
  `QuotaLedger`, confirmed reset and schedule-correction candidates, per-IANA
  natural-day attribution, exact current-epoch seven-date projection, semantic
  ledger rows, account-isolated recovery, and one revisioned public snapshot.
- Persistence and migration: SQLite v4 makes observations immutable and
  epoch-linked, upgrades populated v2/v3 databases without rewriting v3,
  atomically rebuilds derived projections, and quarantines legacy observations
  that do not satisfy the newer strict time-window invariant. Quarantine also
  reconciles source health so an older or empty quota is never reported fresh.
- Boundary safety: summary reset fields and ledger dates use the same confirmed
  active-epoch boundary. Large schedule changes and early resets require two
  coherent observations; request intervals crossing reset accept either the
  valid old-window start or new-window completion instant.
- Verification: Rust fmt, strict workspace Clippy and full tests; UI
  lint/typecheck/18 tests/production build; legacy Node checks/27 tests;
  identity/version checks; offline cargo-deny; generated-binding integrity; and
  a release macOS `QuotaTide.app` bundle all passed.
- Review: all findings from iterative Spec and Standards review were fixed;
  both final static re-reviews passed on `3c930b4..7aba94c`.
