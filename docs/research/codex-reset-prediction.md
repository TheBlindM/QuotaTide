# Codex Runway 重置预测接口调研

复查时间：2026-07-31（Asia/Shanghai）

## 结论

QuotaTide 使用 Codex Runway 发布的匿名静态状态源：

```http
GET https://www.codexrunway.com/api/status.json
Accept: application/json
```

首页把该地址作为原始 JSON feed 暴露。项目仓库说明 feed 由每小时监控任务生成，
只分析 @thsottiaux 的公开动态，不接收 QuotaTide 的账号、Token、auth.json 路径或
本机会话数据。

来源：

- [Codex Runway Reset Watch](https://www.codexrunway.com/)
- [实时 JSON feed](https://www.codexrunway.com/api/status.json)
- [Licoy/codex-runway](https://github.com/Licoy/codex-runway)

## 当前契约

顶层结构：

```json
{
  "schemaVersion": 1,
  "generatedAt": "2026-07-31T01:16:52.250Z",
  "lastSuccessfulCheckAt": "2026-07-31T01:16:52.250Z",
  "monitor": {
    "status": "ok",
    "errorCode": null
  },
  "events": []
}
```

每个事件包含：

| 字段 | 类型 | QuotaTide 用法 |
|---|---|---|
| `kind` | string | 只消费 `reset_scheduled` 与 `reset_completed` |
| `announcedAt` | ISO 8601 | 公开信号出现时间 |
| `effectiveAt` | ISO 8601/null | 计划重置时间；计划事件必须存在且晚于公告时间 |
| `source.handle` | string | 必须为 `thsottiaux` |
| `source.postId` | string | 本地去重与来源校验 |
| `source.url` | URL | 必须是匹配 post ID 的 `https://x.com/thsottiaux/status/...` |
| `confidence` | number | 第三方 AI 分类置信度，必须在 0–1 |
| `rationale` | string | 简短分类理由，不作为 OpenAI 官方说明 |

`scope` 描述计划与额度窗口范围，但 v1 的单账号监控不据此推断个人账号一定会重置。

## 映射规则

- `reset_scheduled`：当 `announcedAt <= now < effectiveAt` 时映射为当前预测。
- 同时存在多个有效计划时，展示 `effectiveAt` 最近的一项。
- `confidence` 进入现有 70% 提醒阈值，但界面必须写“置信度”，不能写成个人重置概率。
- `reset_completed`：取公告时间最新且不在未来的一项作为最近重置公告。
- `limit_increase`：不等于重置，忽略。
- 未识别的新事件类型：为前向兼容而忽略。
- `monitor.status != "ok"`：视为来源失败，保留仍未过期的最后有效快照并显示 stale。
- `generatedAt` 比本机时间超前超过 5 分钟：视为契约错误；比本机时间落后超过
  6 小时：视为上游不可用，防止 CDN 的陈旧文档被误标为 fresh。
- JSON、时间、来源 URL 或数值不满足契约：视为契约错误，不展示伪造数据。

## 隐私与刷新

客户端每小时最多拉取一次该公开静态 JSON。请求不携带 Codex 凭据、账号 ID、
auth.json 内容、邮箱设置或本机会话数据。该预测是第三方、非官方、尽力而为的公开
信号分类，只用于提醒，不覆盖账号自己的官方额度窗口。

## 历史说明

2026-07-28 的早期原型曾读取 `codex-resets.com/api/resets` 的 `watch` 概率对象。
自 2026-07-31 起该契约不再是 QuotaTide 的实现来源；保留这条说明仅用于解释旧测试
夹具和文档历史，不应在新代码或用户界面中继续引用旧域名。
