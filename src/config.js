import path from "node:path";

function numberFromEnv(name, fallback) {
  const raw = process.env[name];
  if (!raw) return fallback;
  const value = Number(raw);
  return Number.isFinite(value) && value > 0 ? value : fallback;
}

function booleanFromEnv(name, fallback) {
  const raw = process.env[name];
  if (raw == null || raw === "") return fallback;
  return ["1", "true", "yes", "on"].includes(raw.toLowerCase());
}

export function loadConfig(cwd = process.cwd()) {
  return {
    host: process.env.HOST || "127.0.0.1",
    port: numberFromEnv("PORT", 4317),
    timezone: process.env.TIMEZONE || "Asia/Shanghai",
    authJsonPath: process.env.AUTH_JSON_PATH || "",
    databasePath: path.resolve(
      cwd,
      process.env.DATABASE_PATH || "./data/quota-monitor.sqlite",
    ),
    pollIntervalMinutes: numberFromEnv("POLL_INTERVAL_MINUTES", 60),
    codexBaseUrl: (process.env.CODEX_BASE_URL || "https://chatgpt.com").replace(
      /\/+$/,
      "",
    ),
    codexResetsUrl:
      process.env.CODEX_RESETS_URL ||
      "https://codex-resets.com/api/resets",
    smtp: {
      host: process.env.SMTP_HOST || "",
      port: numberFromEnv("SMTP_PORT", 465),
      secure: booleanFromEnv("SMTP_SECURE", true),
      user: process.env.SMTP_USER || "",
      pass: process.env.SMTP_PASS || "",
      from: process.env.MAIL_FROM || "",
      to: (process.env.MAIL_TO || "")
        .split(",")
        .map((item) => item.trim())
        .filter(Boolean),
    },
  };
}
