import { useEffect, useState } from "preact/hooks";

import type { BuildInfo } from "./bindings/BuildInfo";
import type { PublicAccountSettings } from "./bindings/PublicAccountSettings";
import type { PublicLedgerDay } from "./bindings/PublicLedgerDay";
import type { PublicLiveQuota } from "./bindings/PublicLiveQuota";
import type { UsageSourceErrorCode } from "./bindings/UsageSourceErrorCode";
import {
  getAccountSettings,
  selectAuthFile,
  saveSettings,
} from "./api/account-settings";
import { loadBuildInfo } from "./api/build-info";
import { getLiveQuota, onDashboardChanged } from "./api/live-quota";
import { hideMainWindow, requestManualRefresh } from "./api/tray-shell";
import { TrayApp } from "./TrayApp";
import {
  ledgerFixtures,
  type LedgerTone,
} from "./WeeklyLedger";

type ViewState =
  | { kind: "loading" }
  | {
      kind: "ready";
      info: BuildInfo;
      accountSettings: PublicAccountSettings;
      liveQuota: PublicLiveQuota | null;
      refreshing: boolean;
    }
  | { kind: "error" };

export function App() {
  const isPreview = new URLSearchParams(window.location.search).has("preview");
  const [state, setState] = useState<ViewState>(
    isPreview
      ? {
          kind: "ready",
          info: {
            productName: "QuotaTide",
            version: "0.1.0",
            author: "TheBlind",
            identifier: "dev.theblind.quotatide",
            stage: "weekly-ledger-preview",
          },
          accountSettings: {
            settingsRevision: 0,
            configured: false,
            pathSummary: null,
            accountLabel: null,
            quotaPolicy: {
              policyRevision: 1,
              policyTimezone: "Asia/Shanghai",
              carryWorkdaysEnabled: true,
              baseMicropoints: [
                16_000_000, 16_000_000, 16_000_000, 16_000_000,
                16_000_000, 10_000_000, 10_000_000,
              ],
            },
          },
          liveQuota: null,
          refreshing: false,
        }
      : { kind: "loading" },
  );

  useEffect(() => {
    if (isPreview) {
      return;
    }

    let active = true;

    void Promise.all([loadBuildInfo(), getAccountSettings(), getLiveQuota()])
      .then(([info, accountSettings, liveQuotaState]) => {
        if (active) {
          setState({
            kind: "ready",
            info,
            accountSettings,
            liveQuota: liveQuotaState.quota,
            refreshing: liveQuotaState.refreshing,
          });
        }
      })
      .catch(() => {
        if (active) {
          setState({ kind: "error" });
        }
      });

    return () => {
      active = false;
    };
  }, [isPreview]);

  useEffect(() => {
    if (isPreview) {
      return;
    }
    let active = true;
    let unlisten: (() => void) | undefined;
    const reloadDashboard = () => {
      void Promise.all([getAccountSettings(), getLiveQuota()])
        .then(([accountSettings, liveQuotaState]) => {
          if (active) {
            setState((current) =>
              current.kind === "ready"
                ? {
                    ...current,
                    accountSettings,
                    liveQuota: liveQuotaState.quota,
                    refreshing: liveQuotaState.refreshing,
                  }
                : current,
            );
          }
        })
        .catch(() => undefined);
    };
    void onDashboardChanged(reloadDashboard).then((dispose) => {
      if (active) {
        unlisten = dispose;
        reloadDashboard();
      } else {
        dispose();
      }
    }).catch(() => undefined);
    return () => {
      active = false;
      unlisten?.();
    };
  }, [isPreview]);

  useEffect(() => {
    const params = new URLSearchParams(window.location.search);
    const theme = params.get("theme");
    const surface = params.get("surface");

    if (theme === "light" || theme === "dark") {
      document.documentElement.dataset.theme = theme;
    } else {
      delete document.documentElement.dataset.theme;
    }

    if (surface === "opaque") {
      document.documentElement.dataset.surface = "opaque";
    } else if (
      document.documentElement.dataset.platformFallback !== "true"
    ) {
      document.documentElement.dataset.surface = "glass";
    }
  }, []);

  if (state.kind === "loading") {
    return <main class="boot-state" aria-busy="true">正在连接 Rust 核心…</main>;
  }

  if (state.kind === "error") {
    return (
      <main>
        <p role="alert">桌面外壳不可用，请从托盘菜单重试。</p>
      </main>
    );
  }

  const requestedState = new URLSearchParams(window.location.search).get("state");
  const tone: LedgerTone =
    requestedState !== null && requestedState in ledgerFixtures
      ? (requestedState as LedgerTone)
      : "fresh";
  const fixture = isPreview
    ? ledgerFixtures[tone]
    : projectLiveFixture(state.accountSettings, state.liveQuota);

  return (
    <TrayApp
      fixture={fixture}
      accountSettings={state.accountSettings}
      externalRefreshing={state.refreshing}
      onHide={() => {
        void hideMainWindow().catch(() => undefined);
      }}
      onRefresh={() => {
        return requestManualRefresh().then(async (cooldownMs) => {
          const liveQuotaState = await getLiveQuota();
          setState((current) =>
            current.kind === "ready"
              ? {
                  ...current,
                  liveQuota: liveQuotaState.quota,
                  refreshing: liveQuotaState.refreshing,
                }
              : current,
          );
          return cooldownMs;
        });
      }}
      onSelectAuth={async (revision) => {
        const accountSettings = await selectAuthFile(revision);
        const liveQuotaState = await getLiveQuota();
        setState((current) =>
          current.kind === "ready"
            ? {
                ...current,
                accountSettings,
                liveQuota: liveQuotaState.quota,
                refreshing: liveQuotaState.refreshing,
              }
            : current,
        );
        return accountSettings;
      }}
      onReloadAccount={getAccountSettings}
      onUpdatePolicy={async (revision, draft) => {
        const accountSettings = await saveSettings(revision, draft);
        const liveQuotaState = await getLiveQuota();
        setState((current) =>
          current.kind === "ready"
            ? {
                ...current,
                accountSettings,
                liveQuota: liveQuotaState.quota,
              }
            : current,
        );
        return accountSettings;
      }}
    />
  );
}

export function projectLiveFixture(
  account: PublicAccountSettings,
  live: PublicLiveQuota | null,
  now = Date.now(),
): (typeof ledgerFixtures)[LedgerTone] {
  if (!account.configured) {
    return ledgerFixtures.unconfigured;
  }
  const base = ledgerFixtures.fresh;
  if (live === null) {
    return {
      ...base,
      tone: "stale",
      weeklyUsed: "",
      weeklyRemaining: "",
      sourceHealth: "Codex 额度 · 等待首次同步",
      windowLabel: "",
      lastSuccess: "尚未成功同步",
      resetAbsolute: "",
      resetRelative: "",
      todayLimit: "",
      radarChance: "",
      days: [],
    };
  }

  const resetMs =
    live.resetsAtUnixS === null ? null : live.resetsAtUnixS * 1000;
  const used = formatMicropoints(live.usedMicropoints);
  const remaining = formatMicropoints(live.remainingMicropoints);
  const sourceHealth = sourceHealthLabel(live);
  const days = live.ledgerDays.map(projectLedgerDay);
  const today = live.ledgerDays.find((day) => day.isToday);
  const todayAvailable = formatMicropoints(live.todayAvailableMicropoints);
  const todayLimit =
    live.todayLimitMicropoints === null ||
    live.todayBaseMicropoints === null ||
    live.todayCarryMicropoints === null
      ? ""
      : live.todayCarryMicropoints > 0
        ? `${formatMicropoints(live.todayLimitMicropoints)}（${formatMicropoints(live.todayBaseMicropoints)} + ${formatMicropoints(live.todayCarryMicropoints)} 动态）`
        : formatMicropoints(live.todayLimitMicropoints);
  const tone =
    live.sourceStatus !== "fresh"
      ? "stale"
      : today?.status === "exceeded"
        ? "over"
        : today?.status === "warning"
          ? "warning"
          : "fresh";
  return {
    ...base,
    tone,
    weeklyUsed: used,
    weeklyRemaining: remaining,
    sourceHealth,
    windowLabel:
      days.length === 0
        ? live.windowStartsAtUnixS === null || live.windowEndsAtUnixS === null
          ? ""
          : `${formatDate(live.windowStartsAtUnixS * 1000)} 至 ${formatDate(live.windowEndsAtUnixS * 1000)}`
        : `${days[0].date} 至 ${days.at(-1)?.date ?? ""}`,
    lastSuccess:
      live.lastSuccessAtUnixMs === null
        ? "尚未成功同步"
        : `上次成功 ${formatDateTime(live.lastSuccessAtUnixMs)}`,
    resetAbsolute: resetMs === null ? "" : formatDateTime(resetMs),
    resetRelative: resetMs === null ? "" : formatRelative(resetMs - now),
    todayAvailable,
    todayLimit,
    radarChance: "",
    days,
  };
}

function projectLedgerDay(day: PublicLedgerDay) {
  const [year, month, date] = day.localDate
    .split("-")
    .map((part) => Number.parseInt(part, 10));
  const naturalDate = new Date(year, month - 1, date);
  const used =
    day.usedMicropoints === null ? null : day.usedMicropoints / 1_000_000;
  const limit = day.limitMicropoints / 1_000_000;
  return {
    label: day.isToday
      ? "今天"
      : new Intl.DateTimeFormat("zh-CN", { weekday: "short" }).format(
          naturalDate,
        ),
    date: `${month.toString().padStart(2, "0")}/${date.toString().padStart(2, "0")}`,
    used,
    limit,
    today: day.isToday,
    status: ledgerStatusLabel(day.status),
  };
}

function ledgerStatusLabel(status: PublicLedgerDay["status"]): string {
  switch (status) {
    case "unknown":
      return "尚无记录";
    case "normal":
      return "进行中";
    case "warning":
      return "接近上限";
    case "exceeded":
      return "已达上限";
    case "finalized":
      return "已封存";
  }
}

function sourceHealthLabel(live: PublicLiveQuota): string {
  switch (live.sourceStatus) {
    case "fresh":
      return "Codex 额度 · 正常";
    case "stale_after_failure":
      return `Codex 额度 · 连续 ${live.consecutiveFailures.toString()} 次失败（${usageErrorLabel(live.publicError)}）`;
    case "stale_by_age":
      return "Codex 额度 · 数据超过 90 分钟";
    case "unavailable":
      return live.consecutiveFailures > 0
        ? `Codex 额度 · 首次同步失败（${usageErrorLabel(live.publicError)}）`
        : "Codex 额度 · 等待首次同步";
  }
}

function usageErrorLabel(error: UsageSourceErrorCode | null): string {
  switch (error) {
    case "auth_path_unavailable":
      return "账号文件不可用";
    case "authentication_stale":
      return "登录已失效";
    case "permission_denied":
      return "访问被拒绝";
    case "rate_limited":
      return "请求过于频繁";
    case "timeout":
      return "请求超时";
    case "upstream_unavailable":
      return "Codex 服务暂不可用";
    case "response_too_large":
    case "invalid_json":
    case "contract_violation":
    case "weekly_window_unavailable":
      return "额度响应暂不可识别";
    case null:
      return "未知原因";
  }
}

function formatMicropoints(value: number | null): string {
  return value === null
    ? ""
    : `${new Intl.NumberFormat(undefined, { maximumFractionDigits: 2 }).format(value / 1_000_000)}%`;
}

function formatDate(value: number): string {
  return new Intl.DateTimeFormat(undefined, {
    month: "2-digit",
    day: "2-digit",
  }).format(value);
}

function formatDateTime(value: number): string {
  return new Intl.DateTimeFormat(undefined, {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(value);
}

function formatRelative(deltaMs: number): string {
  const minutes = Math.round(deltaMs / 60_000);
  if (Math.abs(minutes) < 60) {
    return new Intl.RelativeTimeFormat(undefined, { numeric: "auto" }).format(
      minutes,
      "minute",
    );
  }
  const hours = Math.round(minutes / 60);
  return new Intl.RelativeTimeFormat(undefined, { numeric: "auto" }).format(
    hours,
    "hour",
  );
}
