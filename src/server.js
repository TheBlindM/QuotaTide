import { createReadStream, existsSync } from "node:fs";
import { createServer } from "node:http";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { loadConfig } from "./config.js";
import { QuotaDatabase } from "./database.js";
import { Mailer } from "./mailer.js";
import { QuotaMonitor } from "./monitor.js";
import { decodeRequestPath } from "./static-path.js";

const rootDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const publicDir = path.join(rootDir, "public");
const config = loadConfig(rootDir);
const database = new QuotaDatabase(config.databasePath, config.timezone);
database.reconcileDerivedData();
const mailer = new Mailer(config.smtp);
const monitor = new QuotaMonitor({ config, database, mailer });

let lastManualRefreshAt = 0;

const mimeTypes = {
  ".html": "text/html; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".png": "image/png",
  ".ico": "image/x-icon",
};

function sendJson(response, status, body) {
  response.writeHead(status, {
    "Content-Type": "application/json; charset=utf-8",
    "Cache-Control": "no-store",
    "X-Content-Type-Options": "nosniff",
  });
  response.end(JSON.stringify(body));
}

function serveStatic(request, response) {
  const url = new URL(request.url, "http://localhost");
  const requested = url.pathname === "/" ? "/index.html" : url.pathname;
  const decodedPath = decodeRequestPath(requested);
  if (!decodedPath.ok) {
    response.writeHead(400, { "Content-Type": "text/plain; charset=utf-8" });
    response.end("Bad request");
    return;
  }
  const decoded = decodedPath.value;
  const filePath = path.resolve(publicDir, `.${decoded}`);
  if (!filePath.startsWith(`${publicDir}${path.sep}`) || !existsSync(filePath)) {
    response.writeHead(404, { "Content-Type": "text/plain; charset=utf-8" });
    response.end("Not found");
    return;
  }
  response.writeHead(200, {
    "Content-Type":
      mimeTypes[path.extname(filePath)] || "application/octet-stream",
    "Cache-Control": "no-store",
    "X-Content-Type-Options": "nosniff",
    "Content-Security-Policy":
      "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self'; connect-src 'self'; frame-ancestors 'none'",
  });
  createReadStream(filePath).pipe(response);
}

const server = createServer(async (request, response) => {
  const url = new URL(request.url, "http://localhost");
  if (request.method === "GET" && url.pathname === "/api/status") {
    sendJson(response, 200, monitor.publicStatus());
    return;
  }
  if (request.method === "GET" && url.pathname === "/health") {
    sendJson(response, 200, { ok: true });
    return;
  }
  if (request.method === "POST" && url.pathname === "/api/refresh") {
    const now = Date.now();
    if (now - lastManualRefreshAt < 30_000) {
      sendJson(response, 429, {
        ok: false,
        error: "刷新过于频繁，请在 30 秒后重试",
      });
      return;
    }
    lastManualRefreshAt = now;
    const result = await monitor.runOnce();
    sendJson(response, result.ok ? 200 : 502, result);
    return;
  }
  if (request.method !== "GET" && request.method !== "HEAD") {
    sendJson(response, 405, { error: "Method not allowed" });
    return;
  }
  serveStatic(request, response);
});

monitor.start();
server.listen(config.port, config.host, () => {
  console.log(
    `Codex 共享额度监控已启动：http://${config.host}:${config.port}`,
  );
  if (!config.authJsonPath) {
    console.log("尚未配置 AUTH_JSON_PATH，页面将显示待配置状态。");
  }
});

function shutdown() {
  monitor.stop();
  server.close(() => {
    database.close();
    process.exit(0);
  });
}

process.on("SIGINT", shutdown);
process.on("SIGTERM", shutdown);
