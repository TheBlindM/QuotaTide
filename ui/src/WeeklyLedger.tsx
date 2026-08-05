import { useEffect, useRef, useState } from "preact/hooks";

import type { AlertEventKind } from "./bindings/AlertEventKind";
import type { AlertTarget } from "./bindings/AlertTarget";
import type { PublicAlertInbox } from "./bindings/PublicAlertInbox";
import type { QuotaPressure } from "./bindings/QuotaPressure";
import type { StoryTheme } from "./bindings/StoryTheme";
import { ThemeToggle, type ColorTheme } from "./color-theme";
import { useI18n } from "./i18n-context";
import type { InterfaceLocale } from "./i18n";

export type LedgerTone =
  | "fresh"
  | "warning"
  | "over"
  | "stale"
  | "unconfigured";

export type LedgerDay = {
  label: string;
  date: string;
  used: number | null;
  limit: number | null;
  today?: boolean;
  status: string;
};

type UsageTone = "normal" | "warning" | "danger" | "unknown";

type TideAction =
  | "idle"
  | "running-right"
  | "running-left"
  | "waving"
  | "jumping"
  | "failed"
  | "waiting"
  | "running"
  | "review";

const CHAMBER_WATER_CAP_RATIO = 0.76;
const CHAMBER_WAVE_WIDTH = 600;
const CHAMBER_WAVE_HEIGHT = 20;
const CHAMBER_WAVE_CENTER_Y = CHAMBER_WAVE_HEIGHT / 2;
const CHAMBER_WAVE_SEGMENTS = 64;
const CHAMBER_WAVE_CYCLE_MS = 2_160;
const CHAMBER_WAVE_PRIMARY_CYCLES = 1.35;
const CHAMBER_WAVE_SECONDARY_CYCLES = 2.4;
const CHAMBER_WAVE_SECONDARY_WEIGHT = 0.28;
const TIDE_ACTION_LOOP_MS: Record<TideAction, number> = {
  idle: 1_200,
  "running-right": 800,
  "running-left": 800,
  waving: 720,
  jumping: 750,
  failed: 1_040,
  waiting: 960,
  running: 840,
  review: 960,
};
const TIDE_ACTION_LOOPS: Record<TideAction, number> = {
  idle: 3,
  "running-right": 3,
  "running-left": 3,
  waving: 3,
  jumping: 3,
  failed: 2,
  waiting: 3,
  running: 3,
  review: 3,
};
const TIDE_ACTIONS: Record<QuotaPressure, readonly TideAction[]> = {
  safe: ["idle", "waving", "idle", "jumping", "idle"],
  warning: ["waiting", "idle", "review", "idle", "running"],
  danger: ["running-left", "idle", "running-right", "idle", "failed"],
  critical: ["failed", "idle", "waiting", "idle", "review"],
  recovery: ["waving", "idle", "jumping", "idle"],
};

function chamberWaveAmplitude(waterLevel: number): number {
  const usedFraction = Math.min(1, Math.max(0, waterLevel / 100));
  const edgeFactor = Math.min(
    1,
    Math.min(usedFraction, 1 - usedFraction) * 4,
  );
  return 4.8 * (0.35 + 0.65 * edgeFactor);
}

function chamberWavePath(phase: number, amplitude: number): string {
  const points = Array.from({ length: CHAMBER_WAVE_SEGMENTS + 1 }, (_, index) => {
    const x = (index / CHAMBER_WAVE_SEGMENTS) * CHAMBER_WAVE_WIDTH;
    const horizontalPosition = (x - CHAMBER_WAVE_WIDTH / 2) / CHAMBER_WAVE_WIDTH;
    const primaryWave = Math.sin(
      horizontalPosition * Math.PI * 2 * CHAMBER_WAVE_PRIMARY_CYCLES + phase,
    );
    const secondaryWave = Math.sin(
      horizontalPosition * Math.PI * 2 * CHAMBER_WAVE_SECONDARY_CYCLES - phase * 1.4,
    ) * CHAMBER_WAVE_SECONDARY_WEIGHT;
    const y = CHAMBER_WAVE_CENTER_Y + amplitude * (primaryWave + secondaryWave);
    return `${index === 0 ? "M" : "L"}${x.toFixed(2)} ${y.toFixed(2)}`;
  });
  return points.join(" ");
}

function usageToneForDay(day: LedgerDay | undefined): UsageTone {
  if (
    day?.used === null ||
    day?.used === undefined ||
    day.limit === null ||
    day.limit <= 0
  ) {
    return "unknown";
  }
  const usageRatio = day.used / day.limit;
  if (usageRatio >= 1) {
    return "danger";
  }
  if (usageRatio >= 0.8) {
    return "warning";
  }
  return "normal";
}

export type RadarAnnouncementFixture = {
  text: string;
  sourceUrl: string;
  announcedAt: string;
};

export type RadarFixture =
  | {
      kind: "active";
      chance: string;
      explanation: string;
      sourceUrl: string;
      timing: string;
      health: string;
      announcement: RadarAnnouncementFixture | null;
    }
  | {
      kind: "empty";
      message: string;
      announcement: RadarAnnouncementFixture | null;
    };

export type LedgerFixture = {
  tone: LedgerTone;
  pressure: QuotaPressure;
  weeklyUsed: string;
  weeklyRemaining: string;
  burnProjection: {
    rate: string;
    projectedUsage: string;
    conclusion: string;
  } | null;
  resetCredits: {
    availableLabel: string;
    expiryLabel: string;
  } | null;
  todayAvailable: string;
  todayLimit: string;
  sourceHealth: string;
  windowLabel: string;
  lastSuccess: string;
  resetAbsolute: string;
  resetRelative: string;
  radar: RadarFixture | null;
  days: LedgerDay[];
};

const freshDays: LedgerDay[] = [
  { label: "周五", date: "07/24", used: 12.8, limit: 16, status: "正常" },
  { label: "周六", date: "07/25", used: 6, limit: 10, status: "正常" },
  { label: "周日", date: "07/26", used: 1, limit: 10, status: "正常" },
  { label: "周一", date: "07/27", used: 11, limit: 16, status: "正常" },
  {
    label: "今天",
    date: "07/28",
    used: 11.4,
    limit: 16.8,
    today: true,
    status: "正常",
  },
  { label: "周三", date: "07/29", used: null, limit: 16.8, status: "尚无记录" },
  { label: "周四", date: "07/30", used: null, limit: 16.4, status: "尚无记录" },
];

const freshFixture: LedgerFixture = {
  tone: "fresh",
  pressure: "safe",
  weeklyUsed: "42%",
  weeklyRemaining: "58%",
  burnProjection: {
    rate: "0.3%/小时",
    projectedUsage: "72%",
    conclusion: "按当前速度，到重置预计使用 72%",
  },
  resetCredits: null,
  todayAvailable: "5.4%",
  todayLimit: "基础 16% + 结转 0.8% = 实际 16.8%",
  sourceHealth: "Codex 额度 · 正常",
  windowLabel: "07/24 至 07/30",
  lastSuccess: "上次成功 10:34",
  resetAbsolute: "周四 10:01",
  resetRelative: "约 2 天后",
  radar: {
    kind: "active",
    chance: ">70%",
    explanation: "未来 24 小时可能出现额外重置。",
    sourceUrl: "https://x.com/thsottiaux/status/2081899343091843463",
    timing: "未来 24 小时",
    health: "数据源正常",
    announcement: {
      text: "ChatGPT Work 与 Codex 用户的用量限制已重置。",
      sourceUrl:
        "https://x.com/thsottiaux/status/2082317452755751098",
      announcedAt: "07/29 12:09",
    },
  },
  days: freshDays,
};

export const ledgerFixtures: Record<LedgerTone, LedgerFixture> = {
  fresh: freshFixture,
  warning: {
    ...freshFixture,
    tone: "warning",
    pressure: "warning",
    weeklyRemaining: "55%",
    todayAvailable: "2.6%",
    days: freshDays.map((day) =>
      day.today
        ? { ...day, used: 14.2, limit: 16.8, status: "预警" }
        : { ...day },
    ),
  },
  over: {
    ...freshFixture,
    tone: "over",
    pressure: "danger",
    weeklyRemaining: "48%",
    todayAvailable: "0%",
    days: freshDays.map((day) =>
      day.today
        ? { ...day, used: 18.2, limit: 16.8, status: "超额" }
        : { ...day },
    ),
  },
  stale: {
    ...freshFixture,
    tone: "stale",
    sourceHealth: "Codex 额度 · 连续 3 次失败",
    lastSuccess: "最后快照 07/28 10:34",
    days: freshDays.map((day) => ({ ...day })),
  },
  unconfigured: {
    tone: "unconfigured",
    pressure: "safe",
    weeklyUsed: "",
    weeklyRemaining: "",
    burnProjection: null,
    resetCredits: null,
    todayAvailable: "",
    todayLimit: "",
    sourceHealth: "尚未连接",
    windowLabel: "",
    lastSuccess: "尚未同步",
    resetAbsolute: "",
    resetRelative: "",
    radar: null,
    days: [],
  },
};

type WeeklyLedgerProps = {
  fixture: LedgerFixture;
  storyTheme?: StoryTheme;
  alerts?: PublicAlertInbox | null;
  focusTarget?: AlertTarget | null;
  focusActivationId?: number | null;
  onDismissAlert?: (eventId: number) => unknown;
  onDismissAllAlerts?: () => unknown;
  onWeekDetailChange?: (expanded: boolean) => unknown;
  onOpenSettings: () => void;
  onRefresh: () => unknown;
  theme?: ColorTheme;
  onToggleTheme?: () => void;
  refreshing?: boolean;
  refreshDisabled?: boolean;
};

const noop = () => undefined;

function runOwnedAction(action: (() => unknown) | undefined): void {
  if (action === undefined) {
    return;
  }
  try {
    void Promise.resolve(action()).catch(() => undefined);
  } catch {
    // The owner retains the unchanged inbox when persistence is unavailable.
  }
}

function liveTime(lastSuccess: string): string {
  return lastSuccess.match(/\b\d{1,2}:\d{2}\b/u)?.[0] ?? "--:--";
}

function localizeKnownValue(value: string, locale: InterfaceLocale): string {
  if (locale === "zh-CN" || value === "") {
    return value;
  }
  const exact: Readonly<Partial<Record<string, string>>> = {
    正常: "Normal",
    预警: "Warning",
    超额: "Exceeded",
    尚无记录: "No record yet",
    进行中: "In progress",
    接近上限: "Approaching limit",
    已达上限: "Limit reached",
    已封存: "Finalized",
    今天: "Today",
    周一: "Mon",
    周二: "Tue",
    周三: "Wed",
    周四: "Thu",
    周五: "Fri",
    周六: "Sat",
    周日: "Sun",
    尚未连接: "Not connected",
    尚未同步: "Not synced yet",
    尚未成功同步: "No successful sync yet",
    "Codex 额度 · 正常": "Codex quota · Healthy",
    "Codex 额度 · 等待首次同步": "Codex quota · Waiting for first sync",
    数据源正常: "Source healthy",
    "Explicit Codex quota reset schedule.": "Explicit Codex quota reset schedule.",
    "Explicit Codex quota reset announcement.": "Explicit Codex quota reset announcement.",
    当前无计划重置信号: "No scheduled reset signal",
    重置数据暂不可用: "Reset data is unavailable",
    当前无有效预测: "No active prediction",
    等待首次雷达同步: "Waiting for the first radar sync",
    预测数据暂不可用: "Prediction data is unavailable",
    显示仍有效的最后快照: "Showing the last valid snapshot",
    "未来 24 小时": "Next 24 hours",
  };
  if (exact[value] !== undefined) {
    return exact[value];
  }
  return value
    .replace("Codex 额度 · 连续 ", "Codex quota · ")
    .replace(" 次失败", " consecutive failures")
    .replace("Codex 额度 · 数据超过 90 分钟", "Codex quota · Over 90 minutes old")
    .replace("Codex 额度 · 首次同步失败", "Codex quota · First sync failed")
    .replace("账号文件不可用", "Account file unavailable")
    .replace("登录已失效", "Sign-in expired")
    .replace("访问被拒绝", "Access denied")
    .replace("请求过于频繁", "Rate limited")
    .replace("请求超时", "Request timed out")
    .replace("Codex 服务暂不可用", "Codex service unavailable")
    .replace("额度响应暂不可识别", "Quota response not recognized")
    .replace("未知原因", "Unknown reason")
    .replaceAll("（", " (")
    .replaceAll("）", ")")
    .replace("数据源暂不可用，显示有效快照", "Source unavailable; showing a valid snapshot")
    .replace(/^有效至 /u, "Valid until ")
    .replace(/ 至 /gu, " to ")
    .replace(/^上次成功 /u, "Last successful sync ");
}

function localizeFixture(
  fixture: LedgerFixture,
  locale: InterfaceLocale,
): LedgerFixture {
  if (locale === "zh-CN") {
    return fixture;
  }
  return {
    ...fixture,
    sourceHealth: localizeKnownValue(fixture.sourceHealth, locale),
    windowLabel: localizeKnownValue(fixture.windowLabel, locale),
    lastSuccess: localizeKnownValue(fixture.lastSuccess, locale),
    todayLimit: fixture.todayLimit
      .replace("基础 ", "Base ")
      .replace(" + 结转 ", " + carry ")
      .replace(" = 实际 ", " = adjusted "),
    burnProjection:
      fixture.burnProjection === null
        ? null
        : {
            ...fixture.burnProjection,
            rate: fixture.burnProjection.rate.replace("/小时", "/h"),
            conclusion: fixture.burnProjection.conclusion
              .replace("按当前速度，到重置预计使用 ", "At this rate, projected usage at reset is ")
              .replace("预计", "Expected to hit the limit ")
              .replace("触顶，早于重置", ", before reset"),
          },
    resetCredits:
      fixture.resetCredits === null
        ? null
        : {
            availableLabel: fixture.resetCredits.availableLabel
              .replace("可用 ", "")
              .replace(" 次", " available"),
            expiryLabel: fixture.resetCredits.expiryLabel
              .replace("最近一枚 ", "Next credit expires in ")
              .replace(" 天后到期", " days"),
          },
    radar:
      fixture.radar === null
        ? null
        : fixture.radar.kind === "active"
          ? {
              ...fixture.radar,
              timing: localizeKnownValue(fixture.radar.timing, locale),
              health: localizeKnownValue(fixture.radar.health, locale),
            }
          : {
              ...fixture.radar,
              message: localizeKnownValue(fixture.radar.message, locale),
            },
    days: fixture.days.map((day) => ({
      ...day,
      label: day.today ? textForLocale(locale, "今天", "Today") : day.label,
      status: localizeKnownValue(day.status, locale),
    })),
  };
}

function textForLocale(
  locale: InterfaceLocale,
  zh: string,
  en: string,
): string {
  return locale === "zh-CN" ? zh : en;
}

const reminderCopy: Record<AlertEventKind, string> = {
  daily_80: "今日额度已达到 80%",
  daily_100: "今日额度已用完",
  weekly_remaining_20: "本周额度仅剩 20%",
  weekly_remaining_10: "本周额度仅剩 10%",
  radar_chance_70: "重置预测置信度已达到 70% 档位",
  quota_reset_confirmed: "额度重置已确认",
  source_failures_3: "额度来源连续采集失败",
};
const reminderCopyEn: Record<AlertEventKind, string> = {
  daily_80: "Today's quota has reached 80%",
  daily_100: "Today's quota is exhausted",
  weekly_remaining_20: "20% weekly quota remaining",
  weekly_remaining_10: "10% weekly quota remaining",
  radar_chance_70: "Reset prediction confidence reached the 70% tier",
  quota_reset_confirmed: "Quota reset confirmed",
  source_failures_3: "Quota source keeps failing",
};

function AlertInbox({
  alerts,
  onDismissAlert,
  onDismissAllAlerts,
}: {
  alerts: PublicAlertInbox | null;
  onDismissAlert?: (eventId: number) => unknown;
  onDismissAllAlerts?: () => unknown;
}) {
  const { locale, text } = useI18n();
  const events = alerts?.events ?? [];
  const permissionUnavailable =
    alerts?.notificationPermissionStatus === "denied" ||
    alerts?.notificationPermissionStatus === "error";
  const deliveryFailed = events.some(
    (event) =>
      event.systemDeliveryState === "retry_wait" ||
      event.systemDeliveryState === "failed",
  );
  return (
    <section
      class="alert-inbox"
      aria-label={text("最近提醒", "Recent alerts")}
    >
      <div class="alert-inbox__heading">
        <div>
          <span>{text("最近提醒", "Recent alerts")}</span>
          <small>
            {text(
              `${String(events.length)} 条`,
              `${String(events.length)} alerts`,
            )}
          </small>
        </div>
        {events.length === 0 ? null : (
          <button
            type="button"
            onClick={() => {
              runOwnedAction(onDismissAllAlerts);
            }}
          >
            {text("清空全部", "Clear all")}
          </button>
        )}
      </div>
      {permissionUnavailable ? (
        <p class="alert-inbox__permission" role="status">
          {text(
            "系统通知未授权，应用内提醒仍会保留。",
            "System notifications are not authorized; in-app alerts are still retained.",
          )}
        </p>
      ) : deliveryFailed ? (
        <p class="alert-inbox__permission" role="status">
          {text(
            "系统通知发送失败，应用内提醒已保留；可稍后重试。",
            "System notification delivery failed; the in-app alert is retained for a later retry.",
          )}
        </p>
      ) : null}
      <div class="alert-inbox__events">
        {events.length === 0 ? (
          <p class="alert-inbox__empty">{text("暂无提醒", "No alerts")}</p>
        ) : null}
        {events.map((event) => {
          const copy = (locale === "zh-CN" ? reminderCopy : reminderCopyEn)[
            event.eventKind
          ];
          return (
            <div class="alert-inbox__event" key={event.eventId}>
              <span aria-hidden="true" />
              <div class="alert-inbox__copy">
                <strong>{copy}</strong>
                <small>
                  {event.localDate ??
                    (event.source === "radar"
                      ? "Reset Radar"
                      : text("当前窗口", "Current window"))}
                </small>
              </div>
              <button
                type="button"
                aria-label={text(`删除提醒：${copy}`, `Delete alert: ${copy}`)}
                onClick={() => {
                  runOwnedAction(() => {
                    onDismissAlert?.(event.eventId);
                  });
                }}
              >
                <svg viewBox="0 0 16 16" aria-hidden="true">
                  <path d="m4.5 4.5 7 7m0-7-7 7" />
                </svg>
              </button>
            </div>
          );
        })}
      </div>
    </section>
  );
}

function RadarAnnouncement({
  announcement,
}: {
  announcement: RadarAnnouncementFixture;
}) {
  const { text } = useI18n();
  return (
    <div class="radar-card__announcement radar-card__announcement--inline">
      <span>{text("最近重置公告", "Latest reset announcement")}</span>
      <p>{announcement.text}</p>
      <a href={announcement.sourceUrl} rel="noreferrer" target="_blank">
        {announcement.announcedAt} · {text("查看公告", "View announcement")}
      </a>
    </div>
  );
}

function RadarCard({
  radar,
  compact = false,
}: {
  radar: RadarFixture | null;
  compact?: boolean;
}) {
  const { text } = useI18n();
  if (radar === null) {
    return null;
  }
  return (
    <section
      id="quota-target-radar"
      class={compact ? "radar-card radar-card--summary" : "radar-card"}
      aria-label={text("重置雷达", "Reset radar")}
      tabIndex={-1}
    >
      {radar.kind === "active" ? (
        <>
          <details class="radar-card__details">
            <summary class="radar-card__header">
              <div>
                <span>
                  {text(
                    "预计重置 · 第三方信号",
                    "Predicted reset · Third-party signal",
                  )}
                </span>
                <small>
                  {radar.timing} · {radar.health}
                </small>
              </div>
              <strong>
                {radar.chance}
                <small>{text("置信度", "confidence")}</small>
              </strong>
            </summary>
            <div class="radar-card__popover">
              <div class="radar-card__body">
                <p>{radar.explanation}</p>
                <small>
                  {text(
                    "第三方 AI 分类 · 非 OpenAI 承诺",
                    "Third-party AI classification · Not an OpenAI commitment",
                  )}
                </small>
                <a href={radar.sourceUrl} rel="noreferrer" target="_blank">
                  {text("查看原始来源", "View original source")}
                </a>
              </div>
            </div>
          </details>
          {radar.announcement === null ? null : (
            <RadarAnnouncement announcement={radar.announcement} />
          )}
        </>
      ) : (
        <>
          <div class="radar-card__header">
            <div>
              <span>{text("重置动态", "Reset activity")}</span>
              <small>{radar.message}</small>
            </div>
            <strong aria-hidden="true">—</strong>
          </div>
          {radar.announcement === null ? null : (
            <RadarAnnouncement announcement={radar.announcement} />
          )}
        </>
      )}
    </section>
  );
}

function percentValue(value: string): number {
  const parsed = Number.parseFloat(value.replace("%", "").replace(",", "."));
  return Number.isFinite(parsed) ? Math.min(100, Math.max(0, parsed)) : 0;
}

const pressureThresholds: Partial<Record<QuotaPressure, number>> = {
  warning: 60,
  danger: 80,
  critical: 95,
};

function pressureReason(
  fixture: LedgerFixture,
  locale: InterfaceLocale,
): string {
  if (fixture.pressure === "safe") {
    return textForLocale(
      locale,
      "当前周额度使用和预测用量均在安全范围内。",
      "Current and projected weekly usage are both within the safe range.",
    );
  }
  if (fixture.pressure === "recovery") {
    return textForLocale(
      locale,
      "额度刚刚重置，正在确认新窗口的用量。",
      "The quota just reset; usage in the new window is being confirmed.",
    );
  }

  const threshold = pressureThresholds[fixture.pressure];
  if (threshold === undefined) {
    return "";
  }
  if (percentValue(fixture.weeklyUsed) >= threshold) {
    const ending =
      fixture.pressure === "warning"
        ? textForLocale(locale, "请留意剩余额度。", "Keep an eye on the remaining quota.")
        : fixture.pressure === "critical"
          ? textForLocale(locale, "额度即将用完。", "The quota is almost exhausted.")
          : textForLocale(locale, "额度快用完了。", "The quota is running low.");
    return textForLocale(
      locale,
      `周额度已用 ${fixture.weeklyUsed}（≥ ${String(threshold)}%），${ending}`,
      `Weekly usage is ${fixture.weeklyUsed} (≥ ${String(threshold)}%). ${ending}`,
    );
  }

  const projectedUsage = fixture.burnProjection?.projectedUsage;
  if (
    projectedUsage !== undefined &&
    percentValue(projectedUsage) >= threshold
  ) {
    const pace =
      fixture.pressure === "warning" ? "当前消耗偏快" : "当前消耗过快";
    return textForLocale(
      locale,
      `${pace}，预测到重置时会用到 ${projectedUsage}（≥ ${String(threshold)}%）。`,
      `Usage is running too fast; projected usage at reset is ${projectedUsage} (≥ ${String(threshold)}%).`,
    );
  }

  return textForLocale(
    locale,
    `周额度使用或重置时预测用量已达到 ${String(threshold)}%。`,
    `Current or projected weekly usage has reached ${String(threshold)}%.`,
  );
}

function pressureLabel(
  pressure: QuotaPressure,
  locale: InterfaceLocale,
): string {
  const labels: Record<QuotaPressure, readonly [string, string]> = {
    safe: ["安全", "Safe"],
    warning: ["提醒", "Warning"],
    danger: ["高压", "Danger"],
    critical: ["临界", "Critical"],
    recovery: ["恢复", "Recovery"],
  };
  const [zh, en] = labels[pressure];
  return locale === "zh-CN" ? zh : en;
}

function QuotaChamber({ fixture }: { fixture: LedgerFixture }) {
  const { locale, text } = useI18n();
  const waterLevel = percentValue(fixture.weeklyUsed);
  const forecastLevel = Math.min(
    100,
    percentValue(fixture.burnProjection?.projectedUsage ?? fixture.weeklyUsed),
  );
  const state = pressureLabel(fixture.pressure, locale);
  const isRecovery = fixture.pressure === "recovery";
  const valveState = isRecovery
    ? text("重置阀已开启", "Reset valve open")
    : text("重置阀尚未解锁", "Reset valve locked");
  const projectionDescription =
    fixture.burnProjection === null
      ? text("预测样本不足", "Not enough samples to forecast")
      : text(
          `速率 ${fixture.burnProjection.rate}。${fixture.burnProjection.conclusion}`,
          `Rate ${fixture.burnProjection.rate}. ${fixture.burnProjection.conclusion}`,
        );
  const actions = TIDE_ACTIONS[fixture.pressure];
  const [actionIndex, setActionIndex] = useState(0);
  const tideAction = actions[actionIndex % actions.length] ?? "idle";
  const waveFillRef = useRef<SVGPathElement>(null);
  const waveLineRef = useRef<SVGPathElement>(null);

  useEffect(() => {
    setActionIndex(0);
  }, [fixture.pressure]);

  useEffect(() => {
    const reduceMotion =
      typeof window.matchMedia === "function" &&
      window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (reduceMotion || actions.length < 2) {
      return undefined;
    }
    const timeoutId = window.setTimeout(() => {
      setActionIndex((current) => (current + 1) % actions.length);
    }, TIDE_ACTION_LOOP_MS[tideAction] * TIDE_ACTION_LOOPS[tideAction]);
    return () => {
      window.clearTimeout(timeoutId);
    };
  }, [actions, tideAction]);

  const waterHeight = waterLevel * CHAMBER_WATER_CAP_RATIO;
  const forecastHeight = forecastLevel * CHAMBER_WATER_CAP_RATIO;
  const waveAmplitude = chamberWaveAmplitude(waterLevel);
  const initialWavePath = chamberWavePath(0, waveAmplitude);
  const liveWavePathRef = useRef(initialWavePath);

  useEffect(() => {
    const reduceMotion =
      typeof window.matchMedia === "function" &&
      window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (reduceMotion || typeof window.requestAnimationFrame !== "function") {
      return undefined;
    }

    let animationFrameId = 0;
    let startedAt: number | null = null;
    const animateWave = (timestamp: number) => {
      startedAt ??= timestamp;
      const elapsed = (timestamp - startedAt) % CHAMBER_WAVE_CYCLE_MS;
      const phase = (elapsed / CHAMBER_WAVE_CYCLE_MS) * Math.PI * 2;
      const linePath = chamberWavePath(phase, waveAmplitude);
      liveWavePathRef.current = linePath;
      waveLineRef.current?.setAttribute("d", linePath);
      waveFillRef.current?.setAttribute(
        "d",
        `${linePath} L${String(CHAMBER_WAVE_WIDTH)} ${String(CHAMBER_WAVE_HEIGHT)} L0 ${String(CHAMBER_WAVE_HEIGHT)} Z`,
      );
      animationFrameId = window.requestAnimationFrame(animateWave);
    };
    animationFrameId = window.requestAnimationFrame(animateWave);
    return () => {
      window.cancelAnimationFrame(animationFrameId);
    };
  }, [waveAmplitude]);

  return (
    <div
      class={`primary-stat quota-chamber pressure-${fixture.pressure}`}
      role="group"
      aria-label={text(
        `周额度压力舱：已用 ${fixture.weeklyUsed}，剩余 ${fixture.weeklyRemaining}，${state}。${fixture.resetRelative}重置。${projectionDescription}`,
        `Weekly quota pressure chamber: ${fixture.weeklyUsed} used, ${fixture.weeklyRemaining} remaining, ${state}. Resets ${fixture.resetRelative}. ${projectionDescription}`,
      )}
      title={projectionDescription}
      style={`--water-level:${String(waterLevel)}%;--water-height:${String(waterHeight)}%;--forecast-level:${String(forecastLevel)}%;--forecast-height:${String(forecastHeight)}%`}
    >
      <div class="quota-chamber__viewport" aria-hidden="true">
        <div class="quota-chamber__valve" title={valveState}>
          <span class="quota-chamber__valve-lock" />
        </div>
        {fixture.burnProjection === null ? null : (
          <div class="quota-chamber__forecast" />
        )}
        <div
          class={`quota-robot quota-robot--${fixture.pressure} quota-robot--action-${tideAction}`}
          data-action={tideAction}
        >
          <span key={`${fixture.pressure}-${String(actionIndex)}-${tideAction}`} class="quota-robot__sprite" />
        </div>
        <div class="quota-water">
          <span class="quota-water__wave" aria-hidden="true">
            <svg viewBox="0 0 600 20" preserveAspectRatio="none">
              <path
                ref={waveFillRef}
                class="quota-water__fill"
                d={`${liveWavePathRef.current} L${String(CHAMBER_WAVE_WIDTH)} ${String(CHAMBER_WAVE_HEIGHT)} L0 ${String(CHAMBER_WAVE_HEIGHT)} Z`}
              />
              <path
                ref={waveLineRef}
                class="quota-water__line"
                d={liveWavePathRef.current}
              />
            </svg>
          </span>
        </div>
        <span
          class="quota-chamber__reset-chip"
          title={fixture.resetAbsolute}
        >
          {isRecovery ? text("排水中", "Draining") : fixture.resetRelative}
        </span>
      </div>
    </div>
  );
}

function siegeState(
  pressure: QuotaPressure,
  locale: InterfaceLocale,
): string {
  const labels: Record<QuotaPressure, readonly [string, string]> = {
    safe: ["防线稳定", "Line secure"],
    warning: ["尸群接近", "Horde approaching"],
    danger: ["防线承压", "Line under pressure"],
    critical: ["最后防线", "Last line"],
    recovery: ["补给抵达", "Supplies arrived"],
  };
  const [zh, en] = labels[pressure];
  return locale === "zh-CN" ? zh : en;
}

function LastSupplyLine({ fixture }: { fixture: LedgerFixture }) {
  const { locale, text } = useI18n();
  const state = siegeState(fixture.pressure, locale);
  const supply = percentValue(fixture.weeklyRemaining);
  const weeklyUsed = 100 - supply;
  const advance = fixture.pressure === "recovery"
    ? 27
    : Number((7 + weeklyUsed * 0.2).toFixed(2));
  const supplyBand = supply <= 10 ? "critical" : supply <= 25 ? "low" : "ready";
  const activeSignal = fixture.radar?.kind === "active"
    ? fixture.radar.chance
    : null;
  const signalState = fixture.pressure === "recovery"
    ? "delivered"
    : activeSignal !== null
      ? "active"
      : "scanning";
  const signal: string = signalState === "delivered"
    ? text("已抵达", "Arrived")
    : signalState === "active"
      ? (activeSignal ?? "—")
      : text("搜寻中", "Scanning");
  const pace = fixture.burnProjection?.rate ?? text("待观测", "Observing");

  return (
    <div
      class={`primary-stat supply-line pressure-${fixture.pressure} supply-${supplyBand}`}
      role="group"
      aria-label={text(
        `七日围城：周补给剩余 ${fixture.weeklyRemaining}，${state}。消耗速度 ${pace}。补给信号 ${signal}。`,
        `Last Supply Line: ${fixture.weeklyRemaining} weekly supplies remain. ${state}. Burn rate ${pace}. Supply signal ${signal}.`,
      )}
      style={`--siege-advance:${String(advance)}%`}
    >
      <div class="supply-line__scene" aria-hidden="true">
        <span class="supply-line__moon" />
        <span class="supply-line__skyline" />
        <span class={`supply-line__radio signal-${signalState}`}>
          <i />
        </span>
        <span class="supply-line__road" />
        <div class="supply-line__horde">
          <span class="siege-zombie siege-zombie--one" />
          <span class="siege-zombie siege-zombie--two" />
          <span class="siege-zombie siege-zombie--three" />
          <span class="siege-zombie siege-zombie--four" />
          <span class="siege-zombie siege-zombie--five" />
          <span class="siege-zombie siege-zombie--six" />
          <span class="siege-zombie siege-zombie--seven" />
          <span class="siege-zombie siege-zombie--eight" />
        </div>
        <span class="supply-line__airdrop" data-testid="supply-airdrop" />
        <div class="supply-line__defenders">
          <span class="siege-defender siege-defender--rear" />
          <span class="siege-defender siege-defender--front" />
          <span
            class="siege-defender siege-defender--rpg"
            data-testid="siege-rpg"
          />
          <span class="siege-muzzle" />
        </div>
        <span class="siege-rocket" data-testid="siege-rocket" />
        <span class="siege-blast" data-testid="siege-blast" />
        <span class="supply-line__barricade" />
        <div class="supply-line__crates">
          <span />
          <span />
          <span />
        </div>
      </div>
      <div class="supply-line__readout">
        <span>{text("周补给", "SUPPLIES")}</span>
        <strong>{fixture.weeklyRemaining}</strong>
        <small>{state}</small>
      </div>
      <div class="supply-line__signal">
        <span>{text("补给信号", "SUPPLY SIGNAL")}</span>
        <strong>{signal}</strong>
      </div>
    </div>
  );
}

export function WeeklyLedger({
  fixture: sourceFixture,
  storyTheme = "rising_water",
  alerts = null,
  focusTarget = null,
  focusActivationId = null,
  onDismissAlert,
  onDismissAllAlerts,
  onWeekDetailChange,
  onOpenSettings,
  onRefresh,
  theme = "light",
  onToggleTheme = noop,
  refreshing = false,
  refreshDisabled = false,
}: WeeklyLedgerProps) {
  const { locale, text } = useI18n();
  const fixture = localizeFixture(sourceFixture, locale);
  const [showWeekDetail, setShowWeekDetail] = useState(false);
  const [showAlerts, setShowAlerts] = useState(false);
  const [inspectedDayIndex, setInspectedDayIndex] = useState<number | null>(
    null,
  );
  const alertCount = alerts?.events.length ?? 0;
  const today = fixture.days.find((day) => day.today) ?? fixture.days.at(0);
  const todayUsageTone = usageToneForDay(today);
  const todayUsageState =
    todayUsageTone === "danger"
      ? text("已超额", "Exceeded")
      : todayUsageTone === "warning"
        ? text("接近上限", "Approaching limit")
        : todayUsageTone === "normal"
          ? text("正常", "Normal")
          : text("等待数据", "Waiting for data");
  const weeklyState = pressureLabel(fixture.pressure, locale);
  const weeklyPressureReason = pressureReason(fixture, locale);
  const toggleWeekDetail = () => {
    setInspectedDayIndex(null);
    setShowWeekDetail((current) => {
      const expanded = !current;
      runOwnedAction(() => onWeekDetailChange?.(expanded));
      return expanded;
    });
  };
  useEffect(
    () => () => {
      runOwnedAction(() => onWeekDetailChange?.(false));
    },
    [onWeekDetailChange],
  );
  useEffect(() => {
    if (focusTarget !== null) {
      document.getElementById(`quota-target-${focusTarget}`)?.focus({
        preventScroll: true,
      });
    }
  }, [focusActivationId, focusTarget]);

  if (fixture.tone === "unconfigured") {
    return (
      <article class="weekly-ledger tone-unconfigured" aria-busy={refreshing}>
        <header
          id="quota-target-source"
          class="ledger-header"
          tabIndex={-1}
        >
          <div class="ledger-wordmark">
            <span class="ledger-mark" aria-hidden="true">Q</span>
            <div>
              <h1>QuotaTide</h1>
              <p role="status" aria-live="polite">
                <span class="ledger-live-line">CODEX · OFFLINE</span>
                <span class="ledger-source-health">{fixture.sourceHealth}</span>
              </p>
            </div>
          </div>
          <div class="ledger-header__actions">
            <ThemeToggle theme={theme} onToggle={onToggleTheme} />
          </div>
        </header>
        <main class="ledger-content unconfigured-content">
          <section class="empty-state">
            <span class="empty-state__mark" aria-hidden="true">
              ◌
            </span>
            <h2>{text("连接 Codex 账号", "Connect a Codex account")}</h2>
            <p>
              {text(
                "选择 Codex 自动维护的 auth.json。QuotaTide 仅在本机读取，不会修改或上传令牌。",
                "Choose the auth.json maintained by Codex. QuotaTide reads it locally and never modifies or uploads tokens.",
              )}
            </p>
            <button type="button" onClick={onOpenSettings}>
              {text("选择 auth.json", "Choose auth.json")}
            </button>
          </section>
          <RadarCard radar={fixture.radar} />
        </main>
        <footer class="ledger-footer ledger-footer--empty">
          <span>{fixture.lastSuccess}</span>
        </footer>
      </article>
    );
  }

  const staleBanner =
    fixture.tone === "stale"
      ? {
          title: text("数据已过期", "Data is stale"),
          detail: text(
            "刷新失败或数据已超过 90 分钟，正在显示最后一次完整快照。",
            "Refresh failed or data is over 90 minutes old. The last complete snapshot is shown.",
          ),
        }
      : null;
  const requestRefresh = () => {
    try {
      const refreshResult = onRefresh();
      if (refreshResult instanceof Promise) {
        void refreshResult.catch(() => undefined);
      }
    } catch {
      // The caller owns refresh error presentation; the last snapshot remains visible.
    }
  };

  return (
    <article
      class={`weekly-ledger tone-${fixture.tone}`}
      aria-busy={refreshing}
    >
      <header
        id="quota-target-source"
        class="ledger-header"
        tabIndex={-1}
      >
        <div class="ledger-wordmark">
          <span class="ledger-mark" aria-hidden="true">Q</span>
          <div>
            <h1>QuotaTide</h1>
            <p role="status" aria-live="polite">
              <span class="ledger-live-line">
                CODEX · {refreshing ? "SYNC" : "LIVE"}{" "}
                {liveTime(fixture.lastSuccess)}
              </span>
              <span class="ledger-source-health">
                {refreshing
                  ? text("Codex 额度 · 正在刷新", "Codex quota · Refreshing")
                  : fixture.sourceHealth}
              </span>
            </p>
          </div>
        </div>
        <div class="ledger-header__actions">
          <ThemeToggle theme={theme} onToggle={onToggleTheme} />
          <button
            type="button"
            aria-label={
              refreshing
                ? text("正在刷新", "Refreshing")
                : refreshDisabled
                  ? text("刷新冷却中", "Refresh cooling down")
                  : text("立即刷新", "Refresh now")
            }
            class={refreshing ? "is-spinning" : undefined}
            disabled={refreshDisabled || refreshing}
            onClick={requestRefresh}
          >
            <svg viewBox="0 0 24 24" aria-hidden="true">
              <path d="M20 6v5h-5" />
              <path d="M18.2 15.6A7 7 0 1 1 19.5 9" />
            </svg>
          </button>
          <button
            type="button"
            class="alert-trigger"
            aria-label={
              showAlerts
                ? text("关闭消息", "Close messages")
                : text("打开消息", "Open messages")
            }
            aria-expanded={showAlerts}
            aria-controls="alert-inbox-popover"
            onClick={() => {
              setShowAlerts((current) => !current);
            }}
          >
            <svg viewBox="0 0 24 24" aria-hidden="true">
              <path d="M18 8a6 6 0 0 0-12 0c0 7-3 7-3 7h18s-3 0-3-7" />
              <path d="M10 19h4" />
            </svg>
            {alertCount === 0 ? null : (
              <span class="alert-trigger__count" aria-hidden="true">
                {alertCount > 99 ? "99+" : alertCount}
              </span>
            )}
          </button>
        </div>
        {showAlerts ? (
          <div
            id="alert-inbox-popover"
            class="alert-popover"
            onKeyDown={(event) => {
              if (event.key === "Escape") {
                event.preventDefault();
                event.stopPropagation();
                setShowAlerts(false);
              }
            }}
          >
            <AlertInbox
              alerts={alerts}
              onDismissAlert={onDismissAlert}
              onDismissAllAlerts={onDismissAllAlerts}
            />
          </div>
        ) : null}
      </header>

      <main class="ledger-content">
        {staleBanner === null ? null : (
          <section class="state-banner tone-stale" role="alert">
            <div>
              <strong>{staleBanner.title}</strong>
              <span>{staleBanner.detail}</span>
            </div>
            <button
              type="button"
              disabled={refreshDisabled}
              onClick={requestRefresh}
            >
              {refreshing
                ? text("正在刷新", "Refreshing")
                : refreshDisabled
                  ? text("冷却中", "Cooling down")
                  : text("重试", "Retry")}
            </button>
          </section>
        )}

        <section
          id="quota-target-today"
          class="ledger-summary command-summary"
          aria-label={text("额度控制台", "Quota console")}
          tabIndex={-1}
        >
          {storyTheme === "last_supply_line" ? (
            <LastSupplyLine fixture={fixture} />
          ) : (
            <QuotaChamber fixture={fixture} />
          )}
          <div class="side-stats">
            <div
              class={`side-stat quota-side-stat side-stat--${todayUsageTone} pressure-${fixture.pressure}`}
              aria-label={text(
                `周剩余 ${fixture.weeklyRemaining}，${weeklyState}；原因：${weeklyPressureReason}；今天还可用 ${fixture.todayAvailable}；用量状态：${todayUsageState}${fixture.todayLimit === "" ? "" : `；实际上限 ${fixture.todayLimit}`}`,
                `Weekly remaining ${fixture.weeklyRemaining}, ${weeklyState}; reason: ${weeklyPressureReason}; available today ${fixture.todayAvailable}; usage status: ${todayUsageState}${fixture.todayLimit === "" ? "" : `; adjusted limit ${fixture.todayLimit}`}`,
              )}
              aria-live="polite"
            >
              <div class="quota-side-stat__row quota-side-stat__row--weekly">
                <span>{text("周剩余", "Weekly remaining")}</span>
                <strong>{fixture.weeklyRemaining}</strong>
                <small
                  class="quota-side-stat__state"
                  title={weeklyPressureReason}
                >
                  {weeklyState}
                </small>
              </div>
              <div class="quota-side-stat__row quota-side-stat__row--today">
                <span>{text("今天还可用", "Available today")}</span>
                <strong>{fixture.todayAvailable}</strong>
              </div>
            </div>
            <RadarCard radar={fixture.radar} compact />
          </div>
        </section>

        <section class="ledger-window" aria-labelledby="window-heading">
          <div class="ledger-window__heading">
            <div>
              <span>{text("本周策略", "This week's policy")}</span>
              <h2 id="window-heading">{fixture.windowLabel}</h2>
            </div>
            <button
              type="button"
              class="ledger-window__toggle"
              aria-expanded={showWeekDetail}
              aria-controls="ledger-week-detail"
              onClick={toggleWeekDetail}
            >
              {showWeekDetail
                ? text("收起明细", "Hide details")
                : text("查看明细", "View details")}
            </button>
          </div>
          <div
            class={[
              "ledger-week-switcher",
              showWeekDetail ? "is-expanded" : "",
            ]
              .filter(Boolean)
              .join(" ")}
          >
            <div
              role="list"
              aria-hidden={showWeekDetail}
              aria-label={`${text("本周策略", "This week's policy")} ${fixture.windowLabel}`}
              class={[
                "ledger-week",
                inspectedDayIndex === null ? "" : "has-inspected-day",
              ]
                .filter(Boolean)
                .join(" ")}
              onMouseLeave={() => {
                setInspectedDayIndex(null);
              }}
            >
              {fixture.days.map((day, index) => (
                <div
                  class={inspectedDayIndex === index ? "is-inspected" : undefined}
                  role="listitem"
                  aria-label={`${day.label} ${day.date} · ${day.status}`}
                  key={day.date}
                >
                  <div
                    class={[
                      "ledger-day",
                      `usage-${usageToneForDay(day)}`,
                      day.today ? "is-today" : "",
                      inspectedDayIndex === index ? "is-inspected" : "",
                    ]
                      .filter(Boolean)
                      .join(" ")}
                    tabIndex={0}
                    aria-describedby={
                      inspectedDayIndex === index
                        ? "ledger-day-inspector"
                        : undefined
                    }
                    onMouseEnter={() => {
                      setInspectedDayIndex(index);
                    }}
                    onFocus={() => {
                      setInspectedDayIndex(index);
                    }}
                    onBlur={() => {
                      setInspectedDayIndex(null);
                    }}
                    onClick={() => {
                      setInspectedDayIndex(index);
                    }}
                    onKeyDown={(event) => {
                      if (event.key === "Escape") {
                        setInspectedDayIndex(null);
                      }
                    }}
                  >
                    {inspectedDayIndex === index ? (
                      <div
                        id="ledger-day-inspector"
                        class="ledger-day-inspector"
                        role="tooltip"
                        aria-label={`${day.label} ${text("额度明细", "quota details")}`}
                      >
                        <div class="ledger-day-inspector__identity">
                          <strong>{day.label}</strong>
                          <small title={`${day.date} · ${day.status}`}>
                            {day.date} · {day.status}
                          </small>
                        </div>
                        <div class="ledger-day-inspector__metrics">
                          <span>
                            <small>{text("已用", "Used")}</small>
                            <strong>
                              {day.used === null
                                ? "—"
                                : `${day.used.toFixed(1)}%`}
                            </strong>
                          </span>
                          <span>
                            <small>{text("上限", "Limit")}</small>
                            <strong>
                              {day.limit === null
                                ? "—"
                                : `${day.limit.toFixed(1)}%`}
                            </strong>
                          </span>
                          <span class="is-available">
                            <small>{text("可用", "Available")}</small>
                            <strong>
                              {day.used === null || day.limit === null
                                ? "—"
                                : `${Math.max(0, day.limit - day.used).toFixed(1)}%`}
                            </strong>
                          </span>
                        </div>
                      </div>
                    ) : (
                      <>
                        <span>{day.label}</span>
                        <progress
                          max={day.limit ?? 100}
                          value={day.used ?? 0}
                          aria-label={`${day.label} ${text("已使用", "used")}`}
                        />
                        <small>{day.date}</small>
                      </>
                    )}
                  </div>
                </div>
              ))}
            </div>
            <div
              id="ledger-week-detail"
              class="ledger-week-detail"
              role="region"
              aria-live="polite"
              aria-hidden={!showWeekDetail}
              aria-label={text("整周额度明细", "Full week quota details")}
            >
              {fixture.resetCredits === null ? null : (
                <aside
                  class="reset-credits-strip"
                  aria-label={text("重置券（只读）", "Reset credits (read-only)")}
                >
                  <span aria-hidden="true">↻</span>
                  <div>
                    <strong>{text("重置券", "Reset credits")}</strong>
                    <small>{fixture.resetCredits.expiryLabel}</small>
                  </div>
                  <strong>{fixture.resetCredits.availableLabel}</strong>
                </aside>
              )}
              <div class="ledger-week-detail__labels" aria-hidden="true">
                <span>{text("日期", "Date")}</span>
                <span>{text("使用进度", "Usage progress")}</span>
                <span>{text("使用", "Usage")}</span>
                <span>{text("上限", "Limit")}</span>
                <span>{text("可用", "Available")}</span>
              </div>
              <div class="ledger-week-detail__list" role="list">
                {fixture.days.map((day, index) => (
                  <div
                    class={[
                      "ledger-week-row",
                      `usage-${usageToneForDay(day)}`,
                      day.today ? "is-today" : "",
                    ]
                      .filter(Boolean)
                      .join(" ")}
                    role="listitem"
                    aria-label={`${day.label} ${day.date} · ${day.status}`}
                    key={day.date}
                    style={{ transitionDelay: `${String(index * 28)}ms` }}
                  >
                    <div class="ledger-week-row__identity">
                      <strong>{day.label}</strong>
                      <small>{day.date}</small>
                    </div>
                    <div class="ledger-week-row__progress">
                      <progress
                        max={day.limit ?? 100}
                        value={day.used ?? 0}
                        aria-label={`${day.label} ${text("已使用", "used")}`}
                      />
                      <small>{day.status}</small>
                    </div>
                    <strong>
                      {day.used === null ? "—" : `${day.used.toFixed(1)}%`}
                    </strong>
                    <strong>
                      {day.limit === null ? "—" : `${day.limit.toFixed(1)}%`}
                    </strong>
                    <strong>
                      {day.used === null || day.limit === null
                        ? "—"
                        : `${Math.max(0, day.limit - day.used).toFixed(1)}%`}
                    </strong>
                  </div>
                ))}
              </div>
            </div>
          </div>
        </section>

      </main>

      <footer class="ledger-footer">
        <button type="button" aria-current="page">
          {text("额度", "Quota")}
        </button>
        <button type="button" onClick={onOpenSettings}>
          {text("设置", "Settings")}
        </button>
        <span>{fixture.lastSuccess}</span>
      </footer>
    </article>
  );
}
