# 隐私说明

QuotaTide 采用本地优先设计，不提供项目方遥测或分析服务。

应用只读取用户选择的 `auth.json`，用于请求当前 Codex 额度；不会修改该文件，
不会把令牌复制进应用数据库，也不会把令牌发送给 QuotaTide 维护者。额度刷新
访问所选账号对应的上游 Codex 用量服务。

重置动态会访问 `https://www.codexrunway.com/api/status.json`；更新检查会访问配置的 GitHub
Release endpoint。这些服务会看到普通网络请求所需的 IP、User-Agent 等信息，
但 QuotaTide 不会在重置雷达或更新请求中加入 Codex token、Account ID、额度
历史、邮箱地址或设备标识。

额度观测、设置、提醒历史和来源健康保存在本地 SQLite 数据库中。SMTP 密码
保存在 macOS Keychain 或 Windows Credential Manager；其他 SMTP 设置和收件
地址属于本地设置。发送邮件提醒时，配置的提醒内容会发送给所选 SMTP 服务与
收件人。

应用可以导出已脱敏的诊断 ZIP，也可清除自身本地数据；绝不会删除或改写
`auth.json`。导出或删除前，请在隐私界面确认当时显示的精确路径。
