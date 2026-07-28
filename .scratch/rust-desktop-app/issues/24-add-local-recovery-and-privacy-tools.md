# 24 — 完成本地数据恢复与隐私工具

**What to build:** 让 QuotaTide 的本地账本、设置和提醒能安全迁移、备份和恢复，
并为用户提供严格脱敏的诊断导出与完整但不越界的本地数据清除。

**Blocked by:** 18 — 建立当前七日额度账本；23 — 实现安全邮件投递

**Status:** ready-for-agent

- [ ] 应用数据目录在 macOS 建立受限 mode、Windows 建立当前用户 DACL；
  无法建立或收紧权限时停止写入并进入恢复界面。
- [ ] SQLite 启用 bundled library、foreign keys、WAL、busy timeout 和一个
  serialized connection thread；WebView 不知道数据库路径。
- [ ] migration 有固定 checksum、逐级前向执行并先创建通过完整性检查的滚动
  备份；失败完全回滚。
- [ ] binary 遇到更新 schema 时只读失败，不降级、覆盖或假装空库。
- [ ] 启动处理 WAL/SHM recovery、quick check、领域不变量、unfinished
  external journal 和 projection rebuild 后才启动 worker。
- [ ] 主库损坏时停止写入并隔离原库，从新到旧验证三份备份，恢复最近有效项后
  重新迁移和完整性检查。
- [ ] 所有备份无效时进入专用恢复 UI，提供重试、打开数据目录、脱敏诊断和
  二次确认清除，不静默创建空库。
- [ ] 日志只记录 allowlisted 字段并轮转到 `5 × 1 MiB` 硬上限。
- [ ] 诊断导出重新序列化 allowlisted DTO，扫描 forbidden fields，不包含
  SQLite、vault dump、原始日志目录或秘密，并清理随机临时目录。
- [ ] “清除全部本地数据”先停止 worker，再确认删除两个 vault slot、
  autostart、SQLite/WAL/备份/恢复副本/日志；任何 vault 删除失败都停止，
  且永远不触碰 `auth.json`。
- [ ] 自动测试覆盖权限失败、每个 migration、WAL crash、备份轮换、组合损坏、
  newer schema、journal recovery、诊断 canary 和 scoped clear。
