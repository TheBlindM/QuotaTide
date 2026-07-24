# Codex 共享额度监控

本地单账号监控服务：每小时读取 Codex `auth.json`，查询真实周额度，按工作日 16% / 周末 10% 计算每日使用阈值，并提供邮件告警和网页看板。

## 快速开始

要求 Node.js 22.13 或更高版本。

```bash
npm install
cp .env.example .env
```

编辑 `.env`，至少设置：

```dotenv
AUTH_JSON_PATH=/你的/auth.json/绝对路径
```

然后启动：

```bash
npm start
```

访问 `http://127.0.0.1:4317`。

`npm start` 和 `npm run dev` 会自动读取项目根目录的 `.env`。

如果访问 ChatGPT 需要本机代理，请同时设置 `HTTP_PROXY`、
`HTTPS_PROXY` 和 `NODE_USE_ENV_PROXY=1`。

## 使用 Docker 运行

项目内的 `compose.yaml` 已配置：

- 页面仅发布到本机 `127.0.0.1:4317`；
- `/Users/benteli/.codex/auth.json` 作为只读 secret 挂载；
- SQLite 数据保存在 Docker named volume；
- 容器通过 `host.docker.internal:7897` 使用本机代理；
- 容器异常退出或 Docker 重启后自动恢复。

启动：

```bash
docker compose up -d --build
```

查看状态：

```bash
docker compose ps
docker compose logs --tail=100 monitor
```

停止服务但保留历史数据：

```bash
docker compose down
```

## auth.json

支持常见的 Codex CLI 嵌套结构和 New API 扁平结构：

```json
{"tokens":{"access_token":"…","account_id":"…"}}
```

```json
{"access_token":"…","account_id":"…"}
```

如果文件中没有 `account_id`，服务会尝试从 access token 的 JWT claim 中读取。服务不会修改该文件。

完整行为见 [服务规范](docs/spec.md)，上游接口依据见 [调研记录](docs/research/codex-quota-sources.md)。
