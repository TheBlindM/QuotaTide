import { mkdirSync } from "node:fs";
import path from "node:path";
import { DatabaseSync } from "node:sqlite";
import {
  calculateDelta,
  dailyLimitFor,
  evaluateDailyPolicy,
  localDateParts,
} from "./policy.js";

export class QuotaDatabase {
  constructor(filePath, timezone) {
    mkdirSync(path.dirname(filePath), { recursive: true });
    this.db = new DatabaseSync(filePath);
    this.timezone = timezone;
    this.db.exec(`
      PRAGMA journal_mode = WAL;
      CREATE TABLE IF NOT EXISTS snapshots (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        captured_at INTEGER NOT NULL,
        used_percent REAL NOT NULL,
        remaining_percent REAL NOT NULL,
        reset_at INTEGER NOT NULL,
        window_seconds INTEGER,
        plan_type TEXT,
        allowed INTEGER NOT NULL,
        reset_credits INTEGER NOT NULL DEFAULT 0
      );
      CREATE TABLE IF NOT EXISTS daily_usage (
        local_date TEXT PRIMARY KEY,
        used_percent REAL NOT NULL DEFAULT 0,
        limit_percent REAL NOT NULL,
        status TEXT NOT NULL,
        updated_at INTEGER NOT NULL
      );
      CREATE TABLE IF NOT EXISTS app_state (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL
      );
      CREATE TABLE IF NOT EXISTS alerts (
        alert_key TEXT PRIMARY KEY,
        type TEXT NOT NULL,
        local_date TEXT NOT NULL,
        created_at INTEGER NOT NULL,
        delivery_status TEXT NOT NULL,
        detail TEXT
      );
    `);
  }

  getState(key, fallback = null) {
    const row = this.db
      .prepare("SELECT value FROM app_state WHERE key = ?")
      .get(key);
    if (!row) return fallback;
    try {
      return JSON.parse(row.value);
    } catch {
      return fallback;
    }
  }

  setState(key, value) {
    this.db
      .prepare(
        `INSERT INTO app_state (key, value) VALUES (?, ?)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value`,
      )
      .run(key, JSON.stringify(value));
  }

  latestSnapshot() {
    const row = this.db
      .prepare("SELECT * FROM snapshots ORDER BY captured_at DESC, id DESC LIMIT 1")
      .get();
    return row
      ? {
          capturedAt: row.captured_at,
          usedPercent: row.used_percent,
          remainingPercent: row.remaining_percent,
          resetAt: row.reset_at,
          windowSeconds: row.window_seconds,
          planType: row.plan_type,
          allowed: Boolean(row.allowed),
          resetCredits: row.reset_credits,
        }
      : null;
  }

  recordSnapshot(usage, capturedAt = Date.now()) {
    const previous = this.latestSnapshot();
    const current = {
      capturedAt,
      usedPercent: usage.usedPercent,
      remainingPercent: usage.remainingPercent,
      resetAt: usage.resetAt,
      windowSeconds: usage.windowSeconds,
      planType: usage.planType,
      allowed: usage.allowed,
      resetCredits: usage.resetCredits,
    };
    const { date } = localDateParts(new Date(capturedAt), this.timezone);
    const limit = dailyLimitFor(new Date(capturedAt), this.timezone);
    const delta = calculateDelta(previous, current);
    const existing = this.db
      .prepare("SELECT used_percent FROM daily_usage WHERE local_date = ?")
      .get(date);
    const dailyUsed = Math.max(0, Number(existing?.used_percent || 0) + delta);
    const policy = evaluateDailyPolicy(dailyUsed, limit);
    const epochChanged = Boolean(
      previous &&
        (previous.resetAt !== current.resetAt ||
          current.usedPercent + 0.01 < previous.usedPercent),
    );

    this.db.exec("BEGIN");
    try {
      this.db
        .prepare(
          `INSERT INTO snapshots (
            captured_at, used_percent, remaining_percent, reset_at,
            window_seconds, plan_type, allowed, reset_credits
          ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .run(
          capturedAt,
          current.usedPercent,
          current.remainingPercent,
          current.resetAt,
          current.windowSeconds,
          current.planType,
          current.allowed ? 1 : 0,
          current.resetCredits,
        );
      this.db
        .prepare(
          `INSERT INTO daily_usage (
            local_date, used_percent, limit_percent, status, updated_at
          ) VALUES (?, ?, ?, ?, ?)
          ON CONFLICT(local_date) DO UPDATE SET
            used_percent = excluded.used_percent,
            limit_percent = excluded.limit_percent,
            status = excluded.status,
            updated_at = excluded.updated_at`,
        )
        .run(date, dailyUsed, limit, policy.status, capturedAt);
      this.db.exec("COMMIT");
    } catch (error) {
      this.db.exec("ROLLBACK");
      throw error;
    }

    this.setState("last_success_at", capturedAt);
    this.setState("consecutive_failures", 0);
    this.setState("account", {
      userId: usage.userId,
      email: usage.email,
      planType: usage.planType,
    });
    return { date, policy, previous, current, epochChanged, delta };
  }

  recordFailure(message, capturedAt = Date.now()) {
    const count = Number(this.getState("consecutive_failures", 0)) + 1;
    this.setState("consecutive_failures", count);
    this.setState("last_failure", { at: capturedAt, message });
    return count;
  }

  recordRadar(radar, capturedAt = Date.now()) {
    const previous = this.getState("radar");
    this.setState("radar", { ...radar, fetchedAt: capturedAt });
    return {
      isNew:
        Boolean(previous?.latest?.id) &&
        Boolean(radar.latest?.id) &&
        radar.latest.id !== previous?.latest?.id,
      previous,
    };
  }

  createAlert({ key, type, date, detail, deliveryStatus = "pending" }) {
    const result = this.db
      .prepare(
        `INSERT OR IGNORE INTO alerts (
          alert_key, type, local_date, created_at, delivery_status, detail
        ) VALUES (?, ?, ?, ?, ?, ?)`,
      )
      .run(key, type, date, Date.now(), deliveryStatus, detail || "");
    return result.changes > 0;
  }

  getAlert(key) {
    return this.db
      .prepare(
        `SELECT alert_key, delivery_status
         FROM alerts WHERE alert_key = ?`,
      )
      .get(key);
  }

  updateAlertDelivery(key, status, detail = "") {
    this.db
      .prepare(
        "UPDATE alerts SET delivery_status = ?, detail = ? WHERE alert_key = ?",
      )
      .run(status, detail, key);
  }

  status(now = Date.now()) {
    const { date } = localDateParts(new Date(now), this.timezone);
    const latest = this.latestSnapshot();
    const todayRow = this.db
      .prepare("SELECT * FROM daily_usage WHERE local_date = ?")
      .get(date);
    const limit = dailyLimitFor(new Date(now), this.timezone);
    const today = evaluateDailyPolicy(todayRow?.used_percent || 0, limit);
    const history = this.db
      .prepare(
        `SELECT local_date, used_percent, limit_percent, status, updated_at
         FROM daily_usage ORDER BY local_date DESC LIMIT 7`,
      )
      .all()
      .map((row) => ({
        date: row.local_date,
        used: row.used_percent,
        limit: row.limit_percent,
        status: row.status,
        updatedAt: row.updated_at,
      }));
    const alerts = this.db
      .prepare(
        `SELECT type, local_date, created_at, delivery_status
         FROM alerts ORDER BY created_at DESC LIMIT 8`,
      )
      .all()
      .map((row) => ({
        type: row.type,
        date: row.local_date,
        createdAt: row.created_at,
        deliveryStatus: row.delivery_status,
      }));
    return {
      latest,
      today,
      history,
      alerts,
      account: this.getState("account", {}),
      radar: this.getState("radar"),
      lastSuccessAt: this.getState("last_success_at"),
      consecutiveFailures: this.getState("consecutive_failures", 0),
      lastFailure: this.getState("last_failure"),
      lastConfirmedReset: this.getState("last_confirmed_reset"),
    };
  }

  close() {
    this.db.close();
  }
}
