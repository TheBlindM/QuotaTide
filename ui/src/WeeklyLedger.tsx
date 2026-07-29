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
  limit: number;
  today?: boolean;
  status: string;
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
  radarChance: string;
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
  todayLimit: "16.8%",
  sourceHealth: "Codex 额度 · 正常",
  windowLabel: "07/24 至 07/30",
  lastSuccess: "上次成功 10:34",
  resetAbsolute: "周四 10:01",
  resetRelative: "约 2 天后",
  radarChance: ">70%",
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
    radarChance: "",
    days: [],
  },
};

type WeeklyLedgerProps = {
  fixture: LedgerFixture;
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
      detail: "已达到今日实际上限的 84%，完整七日数据仍保留。",
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

export function WeeklyLedger({
  fixture,
  onOpenSettings,
  onRefresh,
  refreshing = false,
  refreshDisabled = false,
}: WeeklyLedgerProps) {
  if (fixture.tone === "unconfigured") {
    return (
      <article class="weekly-ledger tone-unconfigured">
        <header class="ledger-header">
          <div>
            <h1>QuotaTide</h1>
            <p>{fixture.sourceHealth}</p>
          </div>
        </header>
        <main class="empty-state">
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
      <header class="ledger-header">
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

        <section class="ledger-summary" aria-label="额度摘要">
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
                <th scope="col">今日实际上限</th>
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
                      max={day.limit}
                      value={day.used ?? 0}
                      aria-label={`${day.label}已使用`}
                    />
                    <span>{day.used === null ? day.status : `${day.used.toFixed(1)}% 已用`}</span>
                  </td>
                  <td>
                    <strong>{day.limit.toFixed(1)}%</strong>
                    <span>{day.status}</span>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>

        {fixture.radarChance === "" ? null : (
          <section class="radar-card" aria-label="重置雷达">
            <div>
              <span>重置雷达 · 第三方预测</span>
              <strong>{fixture.radarChance}</strong>
              <small>未来 24 小时</small>
            </div>
            <a href="https://codex-resets.com/">查看来源</a>
          </section>
        )}
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
