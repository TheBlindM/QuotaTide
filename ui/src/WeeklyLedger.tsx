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
  weeklyRemaining: string;
  todayAvailable: string;
  sourceHealth: string;
  windowLabel: string;
  lastSuccess: string;
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
  weeklyRemaining: "58%",
  todayAvailable: "5.4%",
  sourceHealth: "Codex 额度 · 正常",
  windowLabel: "07/24 至 07/30",
  lastSuccess: "上次成功 10:34",
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
    weeklyRemaining: "",
    todayAvailable: "",
    sourceHealth: "尚未连接",
    windowLabel: "",
    lastSuccess: "尚未同步",
    radarChance: "",
    days: [],
  },
};

type WeeklyLedgerProps = {
  fixture: LedgerFixture;
  onOpenSettings: () => void;
  onRefresh: () => void;
};

export function WeeklyLedger({
  fixture,
  onOpenSettings,
  onRefresh,
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

  return (
    <article class={`weekly-ledger tone-${fixture.tone}`}>
      <header class="ledger-header">
        <div>
          <h1>QuotaTide</h1>
          <p>{fixture.sourceHealth}</p>
        </div>
        <div class="ledger-header__actions">
          <button type="button" aria-label="立即刷新" onClick={onRefresh}>
            ↻
          </button>
          <button type="button" aria-label="打开设置" onClick={onOpenSettings}>
            ⚙
          </button>
        </div>
      </header>

      <main class="ledger-content">
        {fixture.tone === "warning" ||
        fixture.tone === "over" ||
        fixture.tone === "stale" ? (
          <section class={`state-banner tone-${fixture.tone}`} role="alert">
            <div>
              <strong>
                {fixture.tone === "over"
                  ? "今日额度已超出"
                  : fixture.tone === "stale"
                    ? "数据已过期"
                    : "接近今日额度"}
              </strong>
              <span>
                {fixture.tone === "over"
                  ? "已超过今日实际上限，完整七日数据仍保留。"
                  : fixture.tone === "stale"
                    ? "连续 3 次刷新失败，正在显示最后一次完整快照。"
                    : "已达到今日实际上限的 84%，完整七日数据仍保留。"}
              </span>
            </div>
            {fixture.tone === "warning" ? (
              <button type="button">查看今日</button>
            ) : fixture.tone === "stale" ? (
              <button type="button" onClick={onRefresh}>
                重试
              </button>
            ) : null}
          </section>
        ) : null}

        <section class="ledger-summary" aria-label="额度摘要">
          <div>
            <span>周剩余</span>
            <strong>{fixture.weeklyRemaining}</strong>
            <small>周四 10:01 重置</small>
          </div>
          <div>
            <span>今天还可用</span>
            <strong>{fixture.todayAvailable}</strong>
            <small>实际上限 16.8%</small>
          </div>
        </section>

        <section class="ledger-window" aria-labelledby="window-heading">
          <div class="ledger-window__heading">
            <div>
              <span>当前七日窗口</span>
              <h2 id="window-heading">{fixture.windowLabel}</h2>
            </div>
            <span class="status-chip">
              {fixture.tone === "warning"
                ? "预警"
                : fixture.tone === "over"
                  ? "超额"
                  : fixture.tone === "stale"
                    ? "数据过期"
                  : "状态良好"}
            </span>
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

        <section class="radar-card" aria-label="重置雷达">
          <div>
            <span>重置雷达 · 第三方预测</span>
            <strong>{fixture.radarChance}</strong>
            <small>未来 24 小时</small>
          </div>
          <a href="https://codex-resets.com/">查看来源</a>
        </section>
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
