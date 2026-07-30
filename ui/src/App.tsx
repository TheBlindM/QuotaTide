import { useEffect, useState } from "preact/hooks";

import type { AlertEventKind } from "./bindings/AlertEventKind";
import type { BuildInfo } from "./bindings/BuildInfo";
import type { PublicAlertInbox } from "./bindings/PublicAlertInbox";
import type { PublicLedgerDay } from "./bindings/PublicLedgerDay";
import type { PublicLiveQuota } from "./bindings/PublicLiveQuota";
import type { PublicResetRadar } from "./bindings/PublicResetRadar";
import type { PublicSettings } from "./bindings/PublicSettings";
import type { UsageSourceErrorCode } from "./bindings/UsageSourceErrorCode";
import {
  getSettings,
  onSettingsChanged,
  saveSettings,
  sendTestEmail,
} from "./api/account-settings";
import {
  getAlerts,
  onAlertsChanged,
  onNotificationOpened,
  requestSystemNotificationPermission,
  type NotificationActivation,
} from "./api/alerts";
import { loadBuildInfo } from "./api/build-info";
import { getLiveQuota, onDashboardChanged } from "./api/live-quota";
import { hideMainWindow, requestManualRefresh } from "./api/tray-shell";
import {
  getStartupState,
  clearAllLocalData,
  exportDiagnostics,
  openLocalDataDirectory,
  retryLocalRecovery,
  type PublicStartupState,
} from "./api/local-data";
import { PrivacyPanel, TrayApp } from "./TrayApp";
import {
  ledgerFixtures,
  type LedgerTone,
} from "./WeeklyLedger";

type ViewState =
  | { kind: "loading" }
  | {
      kind: "ready";
      info: BuildInfo;
      settings: PublicSettings;
      alerts: PublicAlertInbox;
      focusRequest: NotificationActivation | null;
      liveQuota: PublicLiveQuota | null;
      radar: PublicResetRadar;
      refreshing: boolean;
      recoveredFromBackup: boolean;
    }
  | { kind: "recovery"; startup: PublicStartupState }
  | { kind: "error" };

const previewAlertKinds: AlertEventKind[] = [
  "daily_80",
  "daily_100",
  "weekly_remaining_20",
  "weekly_remaining_10",
  "radar_chance_70",
  "quota_reset_confirmed",
  "source_failures_3",
];

export function App() {
  const isPreview = new URLSearchParams(window.location.search).has("preview");
  const previewRecovery = new URLSearchParams(window.location.search).get(
    "recovery",
  );
  const previewAlerts =
    new URLSearchParams(window.location.search).get("alerts") === "denied";
  const [state, setState] = useState<ViewState>(
    isPreview
      ? previewRecovery !== null
        ? {
            kind: "recovery",
            startup: {
              mode:
                previewRecovery === "version"
                  ? "unsupported_schema"
                  : previewRecovery === "permission"
                    ? "storage_permission_denied"
                    : "recovery_required",
              messageKey: "startup.preview",
              recoveredFromBackup: false,
            },
          }
        : {
          kind: "ready",
          info: {
            productName: "QuotaTide",
            version: "0.1.0",
            author: "TheBlind",
            identifier: "dev.theblind.quotatide",
            stage: "weekly-ledger-preview",
          },
          settings: {
            settingsRevision: 0,
            configured: false,
            pathSummary: null,
            accountLabel: null,
            notificationPermissionStatus: previewAlerts ? "denied" : "unknown",
            quotaPolicy: {
              policyRevision: 1,
              policyTimezone: "Asia/Shanghai",
              carryWorkdaysEnabled: true,
              baseMicropoints: [
                16_000_000, 16_000_000, 16_000_000, 16_000_000,
                16_000_000, 10_000_000, 10_000_000,
              ],
            },
            alertPreferences: previewAlertKinds.flatMap((eventKind) => [
              { eventKind, channel: "system", enabled: true },
              { eventKind, channel: "email", enabled: false },
            ]),
            autostartEnabled: false,
            smtp: {
              enabled: false,
              host: "",
              port: 465,
              tlsMode: "tls",
              username: "",
              fromAddress: "",
              fromName: "",
              recipients: [],
              credentialStatus: "missing",
            },
          },
          alerts: previewAlerts
            ? {
                notificationPermissionStatus: "denied",
                events: [
                  {
                    eventId: 1,
                    eventKind: "daily_80",
                    localDate: "2026-07-30",
                    source: null,
                    target: "today",
                    systemDeliveryState: "paused_permission",
                    createdAtUnixMs: 1_785_347_200_000,
                  },
                ],
              }
            : { notificationPermissionStatus: "unknown", events: [] },
          focusRequest: previewAlerts
            ? { target: "today", activationId: 1 }
            : null,
          liveQuota: null,
          radar: emptyRadarState,
          refreshing: false,
          recoveredFromBackup: false,
          }
      : { kind: "loading" },
  );

  useEffect(() => {
    if (isPreview) {
      return;
    }

    let active = true;

    void getStartupState()
      .then(async (startup) => {
        if (startup.mode !== "ready") {
          if (active) {
            setState({ kind: "recovery", startup });
          }
          return;
        }
        const [info, settings, liveQuotaState, alerts] = await Promise.all([
          loadBuildInfo(),
          getSettings(),
          getLiveQuota(),
          getAlerts(),
        ]);
        if (active) {
          setState({
            kind: "ready",
            info,
            settings,
            alerts,
            focusRequest: null,
            liveQuota: liveQuotaState.quota,
            radar: liveQuotaState.radar,
            refreshing: liveQuotaState.refreshing,
            recoveredFromBackup: startup.recoveredFromBackup,
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
    if (isPreview || state.kind !== "ready") {
      return;
    }
    let active = true;
    let unlistenDashboard: (() => void) | undefined;
    let unlistenSettings: (() => void) | undefined;
    let unlistenNotification: (() => void) | undefined;
    let unlistenAlerts: (() => void) | undefined;
    const reloadDashboard = () => {
      void Promise.all([getSettings(), getLiveQuota(), getAlerts()])
        .then(([settings, liveQuotaState, alerts]) => {
          if (active) {
            setState((current) =>
              current.kind === "ready"
                ? {
                    ...current,
                    settings,
                    alerts,
                    liveQuota: liveQuotaState.quota,
                    radar: liveQuotaState.radar,
                    refreshing: liveQuotaState.refreshing,
                  }
                : current,
            );
          }
        })
        .catch(() => undefined);
    };
    void Promise.all([
      onDashboardChanged(reloadDashboard),
      onSettingsChanged(reloadDashboard),
      onAlertsChanged(reloadDashboard),
      onNotificationOpened((activation) => {
        if (active) {
          setState((current) =>
            current.kind === "ready"
              ? { ...current, focusRequest: activation }
              : current,
          );
        }
      }),
    ])
      .then(
        ([
          disposeDashboard,
          disposeSettings,
          disposeAlerts,
          disposeNotification,
        ]) => {
        if (active) {
          unlistenDashboard = disposeDashboard;
          unlistenSettings = disposeSettings;
          unlistenAlerts = disposeAlerts;
          unlistenNotification = disposeNotification;
        } else {
          disposeDashboard();
          disposeSettings();
          disposeAlerts();
          disposeNotification();
        }
        },
      )
      .catch(() => undefined);
    return () => {
      active = false;
      unlistenDashboard?.();
      unlistenSettings?.();
      unlistenAlerts?.();
      unlistenNotification?.();
    };
  }, [isPreview, state.kind]);

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

  if (state.kind === "recovery") {
    return (
      <RecoveryView
        startup={state.startup}
        onOpenData={() => openLocalDataDirectory()}
        onRetry={() => retryLocalRecovery()}
        onHide={() => hideMainWindow()}
        onExportDiagnostics={() => exportDiagnostics()}
        onClearLocalData={() => clearAllLocalData()}
      />
    );
  }

  const requestedState = new URLSearchParams(window.location.search).get("state");
  const previewRadar = new URLSearchParams(window.location.search).get("radar");
  const tone: LedgerTone =
    requestedState !== null && requestedState in ledgerFixtures
      ? (requestedState as LedgerTone)
      : "fresh";
  const fixture = isPreview
    ? {
        ...ledgerFixtures[tone],
        radar:
          previewRadar === "active"
            ? ledgerFixtures.fresh.radar
            : ledgerFixtures[tone].radar,
      }
    : projectLiveFixture(
        state.settings,
        state.liveQuota,
        Date.now(),
        state.radar,
      );

  return (
    <TrayApp
      fixture={fixture}
      settings={state.settings}
      alerts={state.alerts}
      focusRequest={state.focusRequest}
      externalRefreshing={state.refreshing}
      recoveredFromBackup={state.recoveredFromBackup}
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
                  radar: liveQuotaState.radar,
                  refreshing: liveQuotaState.refreshing,
                }
              : current,
          );
          return cooldownMs;
        });
      }}
      onRequestNotificationPermission={requestSystemNotificationPermission}
      onSendTestEmail={sendTestEmail}
      onExportDiagnostics={exportDiagnostics}
      onClearLocalData={clearAllLocalData}
      onReloadSettings={getSettings}
      onSaveSettings={async (draft) => {
        const settings = await saveSettings(draft);
        const liveQuotaState = await getLiveQuota();
        setState((current) =>
          current.kind === "ready"
            ? {
                ...current,
                settings,
                liveQuota: liveQuotaState.quota,
                radar: liveQuotaState.radar,
                refreshing: liveQuotaState.refreshing,
              }
            : current,
        );
        return settings;
      }}
    />
  );
}

function RecoveryView({
  startup,
  onOpenData,
  onRetry,
  onHide,
  onExportDiagnostics,
  onClearLocalData,
}: {
  startup: PublicStartupState;
  onOpenData: () => Promise<void>;
  onRetry: () => Promise<void>;
  onHide: () => Promise<void>;
  onExportDiagnostics: () => Promise<boolean>;
  onClearLocalData: () => Promise<void>;
}) {
  const [busy, setBusy] = useState(false);
  const [actionFailed, setActionFailed] = useState(false);
  const content =
    startup.mode === "unsupported_schema"
      ? {
          eyebrow: "版本保护",
          title: "本地数据来自更新版本",
          detail:
            "QuotaTide 已保持只读，没有降级或覆盖数据。请安装兼容版本后重试。",
        }
      : startup.mode === "storage_permission_denied"
        ? {
            eyebrow: "隐私保护",
            title: "无法保护本地数据目录",
            detail:
              "应用已停止数据库写入。请检查目录所有者与权限，修复后再重试。",
          }
        : {
            eyebrow: "恢复模式",
            title: "本地账本需要处理",
            detail:
              "有效备份已自动尝试。当前没有可安全使用的数据副本，因此没有创建空账本。",
          };

  const run = async (action: () => Promise<void>) => {
    setBusy(true);
    setActionFailed(false);
    try {
      await action();
    } catch {
      setActionFailed(true);
    } finally {
      setBusy(false);
    }
  };

  return (
    <main class="recovery-shell">
      <article class="recovery-card">
        <header class="recovery-header">
          <span class="recovery-orb" aria-hidden="true" />
          <div>
            <p class="recovery-eyebrow">{content.eyebrow}</p>
            <h1>{content.title}</h1>
          </div>
          <button
            type="button"
            class="icon-button"
            aria-label="隐藏窗口"
            onClick={() => void run(onHide)}
          >
            ×
          </button>
        </header>
        <p class="recovery-detail">{content.detail}</p>
        <div class="recovery-note">
          <strong>你的 auth.json 不在处理范围内</strong>
          <span>恢复与清除操作只会作用于 QuotaTide 自己的应用数据。</span>
        </div>
        <div class="recovery-privacy-tools">
          <PrivacyPanel
            onExportDiagnostics={onExportDiagnostics}
            onClearLocalData={onClearLocalData}
          />
        </div>
        {actionFailed ? (
          <p class="settings-error" role="alert">
            操作未完成，请检查系统权限后重试。
          </p>
        ) : null}
        <footer class="recovery-actions">
          <button
            type="button"
            disabled={busy}
            onClick={() => void run(onOpenData)}
          >
            打开数据目录
          </button>
          <button
            type="button"
            class="settings-save"
            disabled={busy}
            onClick={() => void run(onRetry)}
          >
            {busy ? "处理中…" : "重试恢复"}
          </button>
        </footer>
      </article>
    </main>
  );
}

export function projectLiveFixture(
  account: Pick<
    PublicSettings,
    "configured" | "quotaPolicy" | "settingsRevision" | "pathSummary" | "accountLabel"
  >,
  live: PublicLiveQuota | null,
  now = Date.now(),
  radar: PublicResetRadar = emptyRadarState,
): (typeof ledgerFixtures)[LedgerTone] {
  if (!account.configured) {
    return {
      ...ledgerFixtures.unconfigured,
      radar: projectRadarFixture(radar),
    };
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
      radar: projectRadarFixture(radar),
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
      : `基础 ${formatMicropoints(live.todayBaseMicropoints)} + 结转 ${formatMicropoints(live.todayCarryMicropoints)} = 实际 ${formatMicropoints(live.todayLimitMicropoints)}`;
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
    radar: projectRadarFixture(radar),
    days,
  };
}

export const emptyRadarState: PublicResetRadar = {
  lastAttemptAtUnixMs: null,
  lastSuccessAtUnixMs: null,
  consecutiveFailures: 0,
  sourceStatus: "unavailable",
  publicError: null,
  prediction: null,
  latestAnnouncement: null,
};

export function projectRadarFixture(radar: PublicResetRadar) {
  const announcement =
    radar.latestAnnouncement === null
      ? null
      : {
          text: radar.latestAnnouncement.text,
          sourceUrl: radar.latestAnnouncement.sourceUrl,
          announcedAt: formatDateTime(
            radar.latestAnnouncement.announcedAtUnixMs,
          ),
        };
  const prediction = radar.prediction;
  if (prediction !== null) {
    const health =
      radar.sourceStatus === "fresh"
        ? "数据源正常"
        : radar.sourceStatus === "stale_after_failure"
          ? `数据源暂不可用，显示有效快照`
          : "显示仍有效的最后快照";
    return {
      kind: "active" as const,
      chance: prediction.displayChance,
      explanation: prediction.explanation,
      sourceUrl: prediction.sourceUrl,
      timing: `有效至 ${formatDateTime(prediction.expiresAtUnixMs)}`,
      health,
      announcement,
    };
  }
  const message =
    radar.sourceStatus === "fresh"
      ? "当前无有效预测"
      : radar.lastAttemptAtUnixMs === null
        ? "等待首次雷达同步"
        : "预测数据暂不可用";
  return {
    kind: "empty" as const,
    message,
    announcement,
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
