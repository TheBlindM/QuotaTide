import { readAuthFile } from "./auth-file.js";
import { fetchCodexUsage } from "./codex-client.js";
import { localDateParts } from "./policy.js";
import { fetchResetRadar } from "./resets-client.js";

function masked(value, visibleStart = 4, visibleEnd = 4) {
  if (!value) return "";
  if (value.length <= visibleStart + visibleEnd) return "***";
  return `${value.slice(0, visibleStart)}…${value.slice(-visibleEnd)}`;
}

function maskedEmail(email) {
  if (!email || !email.includes("@")) return "";
  const [name, domain] = email.split("@");
  return `${name.slice(0, 2)}***@${domain}`;
}

export class QuotaMonitor {
  constructor({ config, database, mailer }) {
    this.config = config;
    this.database = database;
    this.mailer = mailer;
    this.running = null;
    this.timer = null;
  }

  async deliverAlert(key, type, date, subject, text) {
    const inserted = this.database.createAlert({ key, type, date, detail: text });
    if (!inserted) return;
    const result = await this.mailer.send(subject, text);
    this.database.updateAlertDelivery(key, result.status, result.detail);
  }

  async refreshRadar() {
    try {
      const radar = await fetchResetRadar(this.config);
      this.database.recordRadar(radar);
      return radar;
    } catch (error) {
      this.database.setState("radar_failure", {
        at: Date.now(),
        message: error.message,
      });
      return this.database.getState("radar");
    }
  }

  async runOnce() {
    if (this.running) return this.running;
    this.running = this.#runOnce();
    try {
      return await this.running;
    } finally {
      this.running = null;
    }
  }

  async #runOnce() {
    const now = Date.now();
    const { date } = localDateParts(new Date(now), this.config.timezone);
    const radar = await this.refreshRadar();
    try {
      const credentials = await readAuthFile(this.config.authJsonPath);
      const usage = await fetchCodexUsage(this.config, credentials);
      const result = this.database.recordSnapshot(usage, now);
      this.database.setState("account_identity", {
        accountId: masked(credentials.accountId),
        email: maskedEmail(usage.email || credentials.email),
        userId: masked(usage.userId),
      });

      if (result.policy.ratio >= 0.8) {
        await this.deliverAlert(
          `${date}:daily-warning`,
          "daily_warning",
          date,
          "Codex 共享账号今日额度达到 80%",
          `今日已使用周额度 ${result.policy.used.toFixed(2)}%，当日建议上限为 ${result.policy.limit}%。`,
        );
      }
      if (result.policy.ratio >= 1) {
        await this.deliverAlert(
          `${date}:daily-exceeded`,
          "daily_exceeded",
          date,
          "Codex 共享账号今日额度已超上限",
          `今日已使用周额度 ${result.policy.used.toFixed(2)}%，已达到或超过当日建议上限 ${result.policy.limit}%。`,
        );
      }

      const announcedAt = Date.parse(radar?.latest?.announcedAt || "");
      const radarCanExplainReset =
        result.epochChanged &&
        radar?.latest?.id &&
        radar.latest.id !==
          this.database.getState("last_confirmed_radar_event_id") &&
        Number.isFinite(announcedAt) &&
        announcedAt >= (result.previous?.capturedAt || 0) - 2 * 60 * 60 * 1000;
      if (radarCanExplainReset) {
        this.database.setState("last_confirmed_radar_event_id", radar.latest.id);
        this.database.setState("last_confirmed_reset", {
          at: now,
          reason: "global_announced_reset",
          event: radar.latest,
        });
        await this.deliverAlert(
          `${radar.latest.id}:reset-confirmed`,
          "reset_confirmed",
          date,
          "Codex 全局额度重置已确认",
          `重置雷达出现新公告，账号额度变化已确认。公告：${radar.latest.text}\n${radar.latest.url}`,
        );
      } else if (result.epochChanged) {
        this.database.setState("last_confirmed_reset", {
          at: now,
          reason: "scheduled_or_upstream_reset",
        });
      }

      return { ok: true, at: now };
    } catch (error) {
      const failures = this.database.recordFailure(error.message, now);
      if (failures >= 3) {
        await this.deliverAlert(
          `${date}:fetch-failed`,
          "fetch_failed",
          date,
          "Codex 额度连续采集失败",
          `额度已连续采集失败 ${failures} 次。最近错误：${error.message}`,
        );
      }
      return { ok: false, at: now, error: error.message };
    }
  }

  publicStatus() {
    const status = this.database.status();
    const identity = this.database.getState("account_identity", {});
    const staleAfterMs = this.config.pollIntervalMinutes * 2 * 60 * 1000;
    const configured = Boolean(this.config.authJsonPath);
    return {
      configured,
      timezone: this.config.timezone,
      pollIntervalMinutes: this.config.pollIntervalMinutes,
      mailEnabled: this.mailer.enabled,
      account: {
        ...identity,
        planType: status.account?.planType || status.latest?.planType || "",
      },
      quota: status.latest,
      today: status.today,
      history: status.history,
      alerts: status.alerts,
      radar: status.radar,
      lastSuccessAt: status.lastSuccessAt,
      stale:
        Boolean(status.lastSuccessAt) &&
        Date.now() - status.lastSuccessAt > staleAfterMs,
      consecutiveFailures: status.consecutiveFailures,
      lastFailure: status.lastFailure,
      lastConfirmedReset: status.lastConfirmedReset,
    };
  }

  start() {
    this.runOnce();
    this.timer = setInterval(
      () => this.runOnce(),
      this.config.pollIntervalMinutes * 60 * 1000,
    );
    this.timer.unref();
  }

  stop() {
    if (this.timer) clearInterval(this.timer);
  }
}
