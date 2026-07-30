import { useEffect } from "preact/hooks";

import type { AlertEventKind } from "./bindings/AlertEventKind";
import type { AlertTarget } from "./bindings/AlertTarget";
import type { PublicAlertInbox } from "./bindings/PublicAlertInbox";

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
  onOpenSettings: () => void;
  onRefresh: () => unknown;
  refreshing?: boolean;
  refreshDisabled?: boolean;
};

type ConfiguredTone = Exclude<LedgerTone, "unconfigured">;

type TonePresentation = {
  chip: string;
  banner?: {
    title: string;
    detail: string;
    action?: "today" | "refresh";
  };
};

const tonePresentations: Record<ConfiguredTone, TonePresentation> = {
  fresh: { chip: "状态良好" },
  warning: {
    chip: "预警",
    banner: {
      title: "接近今日额度",
      detail: "已接近今日实际上限，完整七日数据仍保留。",
      action: "today",
    },
  },
  over: {
    chip: "超额",
    banner: {
      title: "今日额度已超出",
      detail: "已超过今日实际上限，完整七日数据仍保留。",
    },
  },
  stale: {
    chip: "数据过期",
    banner: {
      title: "数据已过期",
      detail: "刷新失败或数据已超过 90 分钟，正在显示最后一次完整快照。",
      action: "refresh",
    },
  },
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

function AlertInbox({ alerts }: { alerts: PublicAlertInbox | null }) {
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
    <section class="alert-inbox" aria-label="最近提醒">
      <div class="alert-inbox__heading">
        <span>最近提醒</span>
        <small>{alerts.events.length} 条</small>
      </div>
      {permissionUnavailable ? (
        <p class="alert-inbox__permission" role="status">
          系统通知未授权，应用内提醒仍会保留。
        </p>
      ) : deliveryFailed ? (
        <p class="alert-inbox__permission" role="status">
          系统通知发送失败，应用内提醒已保留；可稍后重试。
        </p>
      ) : null}
      <div class="alert-inbox__events">
        {alerts.events.slice(0, 3).map((event) => (
          <div class="alert-inbox__event" key={event.eventId}>
            <span aria-hidden="true" />
            <strong>{reminderCopy[event.eventKind]}</strong>
            <small>
              {event.localDate ??
                (event.source === "radar" ? "Reset Radar" : "当前窗口")}
            </small>
          </div>
        ))}
      </div>
    </section>
  );
}

function RadarCard({ radar }: { radar: RadarFixture | null }) {
  if (radar === null) {
    return null;
  }
  return (
    <section
      id="quota-target-radar"
      class="radar-card"
      aria-label="重置雷达"
      tabIndex={-1}
    >
      <div class="radar-card__header">
        <div>
          <span>重置雷达 · 第三方预测</span>
          <small>第三方 AI 估算 · 非 OpenAI 承诺</small>
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
            查看原始来源
          </a>
        </div>
      ) : (
        <p class="radar-card__empty">{radar.message}</p>
      )}
      {radar.announcement === null ? null : (
        <div class="radar-card__announcement">
          <span>最近一次全局额外重置公告</span>
          <p>{radar.announcement.text}</p>
          <a
            href={radar.announcement.sourceUrl}
            rel="noreferrer"
            target="_blank"
          >
            {radar.announcement.announcedAt} · 查看公告
          </a>
        </div>
      )}
    </section>
  );
}

export function WeeklyLedger({
  fixture,
  alerts = null,
  focusTarget = null,
  onOpenSettings,
  onRefresh,
  refreshing = false,
  refreshDisabled = false,
}: WeeklyLedgerProps) {
  useEffect(() => {
    if (focusTarget !== null) {
      document.getElementById(`quota-target-${focusTarget}`)?.focus({
        preventScroll: true,
      });
    }
  }, [focusTarget]);

  if (fixture.tone === "unconfigured") {
    return (
      <article class="weekly-ledger tone-unconfigured">
        <header
          id="quota-target-source"
          class="ledger-header"
          tabIndex={-1}
        >
          <div>
            <h1>QuotaTide</h1>
            <p>{fixture.sourceHealth}</p>
          </div>
        </header>
        <main class="ledger-content unconfigured-content">
          <section class="empty-state">
            <span class="empty-state__mark" aria-hidden="true">
              ◌
            </span>
            <h2>连接 Codex 账号</h2>
            <p>
              选择 Codex 自动维护的 auth.json。QuotaTide 仅在本机读取，不会修改或上传令牌。
            </p>
            <button type="button" onClick={onOpenSettings}>
              选择 auth.json
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

  const presentation = tonePresentations[fixture.tone];
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
    <article class={`weekly-ledger tone-${fixture.tone}`}>
      <header
        id="quota-target-source"
        class="ledger-header"
        tabIndex={-1}
      >
        <div>
          <h1>QuotaTide</h1>
          <p>{refreshing ? "Codex 额度 · 正在刷新" : fixture.sourceHealth}</p>
        </div>
        <div class="ledger-header__actions">
          <button
            type="button"
            aria-label={
              refreshing
                ? "正在刷新"
                : refreshDisabled
                  ? "刷新冷却中"
                  : "立即刷新"
            }
            class={refreshing ? "is-spinning" : undefined}
            disabled={refreshDisabled || refreshing}
            onClick={requestRefresh}
          >
            ↻
          </button>
          <button type="button" aria-label="打开设置" onClick={onOpenSettings}>
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
              <button type="button">查看今日</button>
            ) : presentation.banner.action === "refresh" ? (
              <button
                type="button"
                disabled={refreshDisabled}
                onClick={requestRefresh}
              >
                {refreshing ? "正在刷新" : refreshDisabled ? "冷却中" : "重试"}
              </button>
            ) : null}
          </section>
        ) : null}

        <AlertInbox alerts={alerts} />

        <section
          id="quota-target-today"
          class="ledger-summary"
          aria-label="额度摘要"
          tabIndex={-1}
        >
          <div>
            <span>周剩余</span>
            <strong>{fixture.weeklyRemaining}</strong>
            <small>
              已用 {fixture.weeklyUsed} · {fixture.resetAbsolute} 重置
            </small>
            <small>{fixture.resetRelative}</small>
          </div>
          <div>
            <span>今天还可用</span>
            <strong>{fixture.todayAvailable}</strong>
            <small>
              {fixture.todayLimit === ""
                ? "等待每日账本"
                : `实际上限 ${fixture.todayLimit}`}
            </small>
          </div>
        </section>

        <section class="ledger-window" aria-labelledby="window-heading">
          <div class="ledger-window__heading">
            <div>
              <span>当前七日窗口</span>
              <h2 id="window-heading">{fixture.windowLabel}</h2>
            </div>
            <span class="status-chip">{presentation.chip}</span>
          </div>
          <table aria-label={`当前七日窗口 ${fixture.windowLabel}`}>
            <thead>
              <tr>
                <th scope="col">日期</th>
                <th scope="col">使用</th>
                <th scope="col">每日上限</th>
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
                      aria-label={`${day.label}已使用`}
                    />
                    <span>{day.used === null ? day.status : `${day.used.toFixed(1)}% 已用`}</span>
                  </td>
                  <td>
                    <strong>
                      {day.limit === null ? "待策略" : `${day.limit.toFixed(1)}%`}
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
          额度
        </button>
        <button type="button" onClick={onOpenSettings}>
          设置
        </button>
        <span>{fixture.lastSuccess}</span>
      </footer>
    </article>
  );
}
