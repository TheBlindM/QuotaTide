import { useEffect } from "preact/hooks";

import type { AlertEventKind } from "./bindings/AlertEventKind";
import type { AlertTarget } from "./bindings/AlertTarget";
import type { PublicAlertInbox } from "./bindings/PublicAlertInbox";
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
  weeklyUsed: string;
  weeklyRemaining: string;
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
  weeklyUsed: "42%",
  weeklyRemaining: "58%",
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
    weeklyUsed: "",
    weeklyRemaining: "",
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
  alerts?: PublicAlertInbox | null;
  focusTarget?: AlertTarget | null;
  focusActivationId?: number | null;
  onOpenSettings: () => void;
  onRefresh: () => unknown;
  refreshing?: boolean;
  refreshDisabled?: boolean;
};

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
    当前无有效预测: "No active prediction",
    等待首次雷达同步: "Waiting for the first radar sync",
    预测数据暂不可用: "Prediction data is unavailable",
    显示仍有效的最后快照: "Showing the last valid snapshot",
    "未来 24 小时": "Next 24 hours",
    "约 2 天后": "in about 2 days",
    "未来 24 小时可能出现额外重置。":
      "An additional reset may happen in the next 24 hours.",
    "ChatGPT Work 与 Codex 用户的用量限制已重置。":
      "ChatGPT Work and Codex usage limits were reset.",
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
    .replace(/^上次成功 /u, "Last successful sync ")
    .replace(/^周一 /u, "Mon ")
    .replace(/^周二 /u, "Tue ")
    .replace(/^周三 /u, "Wed ")
    .replace(/^周四 /u, "Thu ")
    .replace(/^周五 /u, "Fri ")
    .replace(/^周六 /u, "Sat ")
    .replace(/^周日 /u, "Sun ");
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
    resetAbsolute: localizeKnownValue(fixture.resetAbsolute, locale),
    resetRelative: localizeKnownValue(fixture.resetRelative, locale),
    todayLimit: fixture.todayLimit
      .replace("基础 ", "Base ")
      .replace(" + 结转 ", " + carry ")
      .replace(" = 实际 ", " = adjusted "),
    radar:
      fixture.radar === null
        ? null
        : fixture.radar.kind === "active"
          ? {
              ...fixture.radar,
              explanation: localizeKnownValue(
                fixture.radar.explanation,
                locale,
              ),
              timing: localizeKnownValue(fixture.radar.timing, locale),
              health: localizeKnownValue(fixture.radar.health, locale),
              announcement:
                fixture.radar.announcement === null
                  ? null
                  : {
                      ...fixture.radar.announcement,
                      text: localizeKnownValue(
                        fixture.radar.announcement.text,
                        locale,
                      ),
                    },
            }
          : {
              ...fixture.radar,
              message: localizeKnownValue(fixture.radar.message, locale),
            },
    days: fixture.days.map((day) => ({
      ...day,
      label: localizeKnownValue(day.label, locale),
      status: localizeKnownValue(day.status, locale),
    })),
  };
}

type TonePresentation = {
  chip: string;
  banner?: {
    title: string;
    detail: string;
    action?: "today" | "refresh";
  };
};

const reminderCopy: Record<AlertEventKind, string> = {
  daily_80: "今日额度已达到 80%",
  daily_100: "今日额度已用完",
  weekly_remaining_20: "本周额度仅剩 20%",
  weekly_remaining_10: "本周额度仅剩 10%",
  radar_chance_70: "重置机会已达到 70% 档位",
  quota_reset_confirmed: "额度重置已确认",
  source_failures_3: "额度来源连续采集失败",
};
const reminderCopyEn: Record<AlertEventKind, string> = {
  daily_80: "Today's quota has reached 80%",
  daily_100: "Today's quota is exhausted",
  weekly_remaining_20: "20% weekly quota remaining",
  weekly_remaining_10: "10% weekly quota remaining",
  radar_chance_70: "Reset chance reached the 70% tier",
  quota_reset_confirmed: "Quota reset confirmed",
  source_failures_3: "Quota source keeps failing",
};

function AlertInbox({ alerts }: { alerts: PublicAlertInbox | null }) {
  const { locale, text } = useI18n();
  if (alerts === null || alerts.events.length === 0) {
    return null;
  }
  const permissionUnavailable =
    alerts.notificationPermissionStatus === "denied" ||
    alerts.notificationPermissionStatus === "error";
  const deliveryFailed = alerts.events.some(
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
        <span>{text("最近提醒", "Recent alerts")}</span>
        <small>
          {text(
            `${String(alerts.events.length)} 条`,
            `${String(alerts.events.length)} alerts`,
          )}
        </small>
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
        {alerts.events.slice(0, 3).map((event) => (
          <div class="alert-inbox__event" key={event.eventId}>
            <span aria-hidden="true" />
            <strong>
              {(locale === "zh-CN" ? reminderCopy : reminderCopyEn)[
                event.eventKind
              ]}
            </strong>
            <small>
              {event.localDate ??
                (event.source === "radar"
                  ? "Reset Radar"
                  : text("当前窗口", "Current window"))}
            </small>
          </div>
        ))}
      </div>
    </section>
  );
}

function RadarCard({ radar }: { radar: RadarFixture | null }) {
  const { text } = useI18n();
  if (radar === null) {
    return null;
  }
  return (
    <section
      id="quota-target-radar"
      class="radar-card"
      aria-label={text("重置雷达", "Reset radar")}
      tabIndex={-1}
    >
      <div class="radar-card__header">
        <div>
          <span>{text("重置雷达 · 第三方预测", "Reset radar · Third-party prediction")}</span>
          <small>
            {text(
              "第三方 AI 估算 · 非 OpenAI 承诺",
              "Third-party AI estimate · Not an OpenAI commitment",
            )}
          </small>
        </div>
        {radar.kind === "active" ? <strong>{radar.chance}</strong> : null}
      </div>
      {radar.kind === "active" ? (
        <div class="radar-card__body">
          <p>{radar.explanation}</p>
          <small>
            {radar.timing} · {radar.health}
          </small>
          <a href={radar.sourceUrl} rel="noreferrer" target="_blank">
            {text("查看原始来源", "View original source")}
          </a>
        </div>
      ) : (
        <p class="radar-card__empty">{radar.message}</p>
      )}
      {radar.announcement === null ? null : (
        <div class="radar-card__announcement">
          <span>
            {text(
              "最近一次全局额外重置公告",
              "Latest global extra-reset announcement",
            )}
          </span>
          <p>{radar.announcement.text}</p>
          <a
            href={radar.announcement.sourceUrl}
            rel="noreferrer"
            target="_blank"
          >
            {radar.announcement.announcedAt} ·{" "}
            {text("查看公告", "View announcement")}
          </a>
        </div>
      )}
    </section>
  );
}

export function WeeklyLedger({
  fixture: sourceFixture,
  alerts = null,
  focusTarget = null,
  focusActivationId = null,
  onOpenSettings,
  onRefresh,
  refreshing = false,
  refreshDisabled = false,
}: WeeklyLedgerProps) {
  const { locale, text } = useI18n();
  const fixture = localizeFixture(sourceFixture, locale);
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
          <div>
            <h1>QuotaTide</h1>
            <p role="status" aria-live="polite">
              {fixture.sourceHealth}
            </p>
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

  const presentation: TonePresentation =
    fixture.tone === "fresh"
      ? { chip: text("状态良好", "Healthy") }
      : fixture.tone === "warning"
        ? {
            chip: text("预警", "Warning"),
            banner: {
              title: text("接近今日额度", "Approaching today's quota"),
              detail: text(
                "已接近今日实际上限，完整七日数据仍保留。",
                "Usage is close to today's adjusted limit; the full seven-day record remains available.",
              ),
              action: "today",
            },
          }
        : fixture.tone === "over"
          ? {
              chip: text("超额", "Exceeded"),
              banner: {
                title: text("今日额度已超出", "Today's quota is exceeded"),
                detail: text(
                  "已超过今日实际上限，完整七日数据仍保留。",
                  "Usage exceeded today's adjusted limit; the full seven-day record remains available.",
                ),
              },
            }
          : {
              chip: text("数据过期", "Stale data"),
              banner: {
                title: text("数据已过期", "Data is stale"),
                detail: text(
                  "刷新失败或数据已超过 90 分钟，正在显示最后一次完整快照。",
                  "Refresh failed or data is over 90 minutes old. The last complete snapshot is shown.",
                ),
                action: "refresh",
              },
            };
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
        <div>
          <h1>QuotaTide</h1>
          <p role="status" aria-live="polite">
            {refreshing
              ? text("Codex 额度 · 正在刷新", "Codex quota · Refreshing")
              : fixture.sourceHealth}
          </p>
        </div>
        <div class="ledger-header__actions">
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
            ↻
          </button>
          <button
            type="button"
            aria-label={text("打开设置", "Open settings")}
            onClick={onOpenSettings}
          >
            ⚙
          </button>
        </div>
      </header>

      <main class="ledger-content">
        {presentation.banner ? (
          <section class={`state-banner tone-${fixture.tone}`} role="alert">
            <div>
              <strong>{presentation.banner.title}</strong>
              <span>{presentation.banner.detail}</span>
            </div>
            {presentation.banner.action === "today" ? (
              <button type="button">{text("查看今日", "View today")}</button>
            ) : presentation.banner.action === "refresh" ? (
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
            ) : null}
          </section>
        ) : null}

        <AlertInbox alerts={alerts} />

        <section
          id="quota-target-today"
          class="ledger-summary"
          aria-label={text("额度摘要", "Quota summary")}
          tabIndex={-1}
        >
          <div>
            <span>{text("周剩余", "Weekly remaining")}</span>
            <strong>{fixture.weeklyRemaining}</strong>
            <small>
              {text("已用", "Used")} {fixture.weeklyUsed} ·{" "}
              {fixture.resetAbsolute} {text("重置", "reset")}
            </small>
            <small>{fixture.resetRelative}</small>
          </div>
          <div>
            <span>{text("今天还可用", "Available today")}</span>
            <strong>{fixture.todayAvailable}</strong>
            <small>
              {fixture.todayLimit === ""
                ? text("等待每日账本", "Waiting for today's ledger")
                : `${text("实际上限", "Adjusted limit")} ${fixture.todayLimit}`}
            </small>
          </div>
        </section>

        <section class="ledger-window" aria-labelledby="window-heading">
          <div class="ledger-window__heading">
            <div>
              <span>{text("当前七日窗口", "Current seven-day window")}</span>
              <h2 id="window-heading">{fixture.windowLabel}</h2>
            </div>
            <span class="status-chip">{presentation.chip}</span>
          </div>
          <table
            aria-label={`${text("当前七日窗口", "Current seven-day window")} ${fixture.windowLabel}`}
          >
            <thead>
              <tr>
                <th scope="col">{text("日期", "Date")}</th>
                <th scope="col">{text("使用", "Usage")}</th>
                <th scope="col">{text("每日上限", "Daily limit")}</th>
              </tr>
            </thead>
            <tbody>
              {fixture.days.map((day) => (
                <tr class={day.today ? "is-today" : undefined} key={day.date}>
                  <th scope="row">
                    <strong>{day.label}</strong>
                    <span>{day.date}</span>
                  </th>
                  <td>
                    <progress
                      max={day.limit ?? 100}
                      value={day.used ?? 0}
                      aria-label={`${day.label} ${text("已使用", "used")}`}
                    />
                    <span>
                      {day.used === null
                        ? day.status
                        : `${day.used.toFixed(1)}% ${text("已用", "used")}`}
                    </span>
                  </td>
                  <td>
                    <strong>
                      {day.limit === null
                        ? text("待策略", "Pending policy")
                        : `${day.limit.toFixed(1)}%`}
                    </strong>
                    <span>{day.status}</span>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>

        <RadarCard radar={fixture.radar} />
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
