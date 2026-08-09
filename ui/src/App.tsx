import { useEffect, useRef, useState } from "preact/hooks";

import type { AlertEventKind } from "./bindings/AlertEventKind";
import type { BuildInfo } from "./bindings/BuildInfo";
import type { PublicAlertInbox } from "./bindings/PublicAlertInbox";
import type { PublicLedgerDay } from "./bindings/PublicLedgerDay";
import type { PublicLiveQuota } from "./bindings/PublicLiveQuota";
import type { PublicResetCredits } from "./bindings/PublicResetCredits";
import type { PublicResetRadar } from "./bindings/PublicResetRadar";
import type { PublicSettings } from "./bindings/PublicSettings";
import type { UsageSourceErrorCode } from "./bindings/UsageSourceErrorCode";
import {
  getSettings,
  onSettingsChanged,
  pickAuthFile,
  saveSettings,
  sendTestEmail,
} from "./api/account-settings";
import {
  dismissAlert,
  dismissAllAlerts,
  getAlerts,
  onAlertsChanged,
  onNotificationOpened,
  requestSystemNotificationPermission,
  type NotificationActivation,
} from "./api/alerts";
import { loadBuildInfo } from "./api/build-info";
import { getLiveQuota, onDashboardChanged } from "./api/live-quota";
import { getResetCredits } from "./api/reset-credits";
import {
  hideMainWindow,
  requestManualRefresh,
  setAccessibleSurface,
  setMainWindowExpanded,
} from "./api/tray-shell";
import {
  getUpdateState,
  installPendingUpdate,
  onUpdateState,
  requestUpdateCheck,
  type PublicUpdateState,
} from "./api/updater";
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
  type LedgerFixture,
} from "./WeeklyLedger";
import { I18nProvider, useI18n } from "./i18n-context";
import {
  createPreviewScenario,
  PREVIEW_NOW_UNIX_MS,
} from "./preview-fixtures";
import {
  formatPercent,
  formatResetTime,
  resolveInterfaceLocale,
  translate,
  type InterfaceLocale,
} from "./i18n";

type ViewState =
  | { kind: "loading" }
  | {
      kind: "ready";
      info: BuildInfo;
      settings: PublicSettings;
      alerts: PublicAlertInbox;
      focusRequest: NotificationActivation | null;
      liveQuota: PublicLiveQuota | null;
      resetCredits: PublicResetCredits | null;
      radar: PublicResetRadar;
      refreshing: boolean;
      recoveredFromBackup: boolean;
      updateState: PublicUpdateState;
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

function useMinuteClock(enabled: boolean): number {
  const [nowUnixMs, setNowUnixMs] = useState(() => Date.now());

  useEffect(() => {
    if (!enabled) {
      return;
    }
    let timeoutId: number | undefined;
    const schedule = () => {
      const now = Date.now();
      const delay = 60_000 - (now % 60_000);
      timeoutId = window.setTimeout(() => {
        setNowUnixMs(Date.now());
        schedule();
      }, delay);
    };
    schedule();
    return () => {
      if (timeoutId !== undefined) {
        window.clearTimeout(timeoutId);
      }
    };
  }, [enabled]);

  return nowUnixMs;
}

function policyLocalDate(nowUnixMs: number, timeZone: string): string {
  const parts = new Intl.DateTimeFormat("en", {
    timeZone,
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  }).formatToParts(nowUnixMs);
  const read = (type: Intl.DateTimeFormatPartTypes) =>
    parts.find((part) => part.type === type)?.value ?? "";
  return `${read("year")}-${read("month")}-${read("day")}`;
}

export function App() {
  const { text } = useI18n();
  const searchParams = new URLSearchParams(window.location.search);
  const isPreview = searchParams.has("preview");
  const isRisingWaterPrototype =
    isPreview && searchParams.get("prototype") === "rising-water";
  const nowUnixMs = useMinuteClock(!isPreview);
  const lastPolicyDateRef = useRef<string | null>(null);
  const previewRecovery = searchParams.get("recovery");
  const previewAlerts = searchParams.get("alerts") === "denied";
  // PROTOTYPE — loops through the complete tide progression without touching
  // real account data. Remove after the rising-water motion is approved.
  const previewSearchParams = new URLSearchParams(searchParams);
  if (isRisingWaterPrototype && !previewSearchParams.has("quota")) {
    previewSearchParams.set("quota", "5");
  }
  const preview = createPreviewScenario(previewSearchParams);
  const prototypeStartingUsedPercent =
    (preview.liveQuota?.usedMicropoints ?? 5_000_000) / 1_000_000;
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
            version: "0.1.1",
            author: "TheBlind",
            identifier: "dev.theblind.quotatide",
            stage: "weekly-ledger-preview",
          },
          settings: {
            settingsRevision: 0,
            configured: preview.configured,
            pathSummary: preview.configured ? "…/auth.json" : null,
            accountLabel: preview.configured ? "Codex • PREVIEW" : null,
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
            autoUpdateEnabled: true,
            trayDisplayMode: "wave",
            storyTheme: searchParams.get("story") ?? "rising_water",
            interfaceLocale: preview.interfaceLocale,
            formatLocale: preview.formatLocale,
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
          liveQuota: preview.liveQuota,
          resetCredits: {
            availableCount: 2,
            credits: [
              {
                status: "available",
                expiresAtUnixS: Math.floor(PREVIEW_NOW_UNIX_MS / 1_000) + 3 * 86_400,
              },
            ],
            checkedAtUnixMs: PREVIEW_NOW_UNIX_MS,
          },
          radar: preview.radar,
          refreshing: false,
          recoveredFromBackup: false,
          updateState: {
            status: "idle",
            currentVersion: "0.1.1",
            availableVersion: null,
            notes: null,
            lastCheckedAtUnixMs: null,
            errorCode: null,
          },
          }
      : { kind: "loading" },
  );
  const systemLocale =
    navigator.languages[0] || navigator.language || "en";
  const platformFallbackLocale = resolveInterfaceLocale(
    state.kind === "ready" ? state.settings.interfaceLocale : "system",
    systemLocale,
  );
  const dashboardRefreshing =
    state.kind === "ready" && state.refreshing;

  useEffect(() => {
    if (!isRisingWaterPrototype || previewRecovery !== null) {
      return undefined;
    }

    let usedPercent = prototypeStartingUsedPercent;
    let peakHoldTicks = 0;
    const intervalId = window.setInterval(() => {
      if (usedPercent >= 98) {
        peakHoldTicks += 1;
        if (peakHoldTicks < 9) {
          return;
        }
        usedPercent = 5;
        peakHoldTicks = 0;
      } else {
        usedPercent += 1;
      }

      const stepParams = new URLSearchParams(previewSearchParams);
      stepParams.set("quota", String(usedPercent));
      const liveQuota = createPreviewScenario(stepParams).liveQuota;
      setState((current) =>
        current.kind === "ready"
          ? { ...current, liveQuota }
          : current,
      );
    }, 180);
    return () => {
      window.clearInterval(intervalId);
    };
  }, [
    isRisingWaterPrototype,
    previewRecovery,
    prototypeStartingUsedPercent,
  ]);

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
        const [info, settings, liveQuotaState, resetCredits, alerts, updateState] =
          await Promise.all([
          loadBuildInfo(),
          getSettings(),
          getLiveQuota(),
          getResetCredits().catch(() => null),
          getAlerts(),
            getUpdateState(),
          ]);
        if (active) {
          setState({
            kind: "ready",
            info,
            settings,
            alerts,
            focusRequest: null,
            liveQuota: liveQuotaState.quota,
            resetCredits,
            radar: liveQuotaState.radar,
            refreshing: liveQuotaState.refreshing,
            recoveredFromBackup: startup.recoveredFromBackup,
            updateState,
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
    let unlistenUpdate: (() => void) | undefined;
    const reloadDashboard = () => {
      void Promise.all([
        getSettings(),
        getLiveQuota(),
        getResetCredits().catch(() => null),
        getAlerts(),
      ])
        .then(([settings, liveQuotaState, resetCredits, alerts]) => {
          if (active) {
            setState((current) =>
              current.kind === "ready"
                ? {
                    ...current,
                    settings,
                    alerts,
                    liveQuota: liveQuotaState.quota,
                    resetCredits,
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
      onUpdateState((updateState) => {
        if (active) {
          setState((current) =>
            current.kind === "ready" ? { ...current, updateState } : current,
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
          disposeUpdate,
        ]) => {
        if (active) {
          unlistenDashboard = disposeDashboard;
          unlistenSettings = disposeSettings;
          unlistenAlerts = disposeAlerts;
          unlistenNotification = disposeNotification;
          unlistenUpdate = disposeUpdate;
          // Close the startup handshake race: a refresh can finish after the
          // initial snapshot but before the native listeners are attached.
          reloadDashboard();
        } else {
          disposeDashboard();
          disposeSettings();
          disposeAlerts();
          disposeNotification();
          disposeUpdate();
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
      unlistenUpdate?.();
    };
  }, [isPreview, state.kind]);

  useEffect(() => {
    if (isPreview || state.kind !== "ready" || !dashboardRefreshing) {
      return;
    }

    let active = true;
    let timeoutId: number | undefined;
    const reconcileRefresh = () => {
      void getLiveQuota()
        .then((liveQuotaState) => {
          if (!active) {
            return;
          }
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
          if (liveQuotaState.refreshing) {
            timeoutId = window.setTimeout(reconcileRefresh, 1_000);
          }
        })
        .catch(() => {
          if (active) {
            timeoutId = window.setTimeout(reconcileRefresh, 1_000);
          }
        });
    };

    timeoutId = window.setTimeout(reconcileRefresh, 1_000);
    return () => {
      active = false;
      if (timeoutId !== undefined) {
        window.clearTimeout(timeoutId);
      }
    };
  }, [dashboardRefreshing, isPreview, state.kind]);

  const currentPolicyDate =
    state.kind === "ready"
      ? policyLocalDate(
          nowUnixMs,
          state.settings.quotaPolicy.policyTimezone,
        )
      : null;
  useEffect(() => {
    if (isPreview || state.kind !== "ready" || currentPolicyDate === null) {
      return;
    }
    const previousPolicyDate = lastPolicyDateRef.current;
    lastPolicyDateRef.current = currentPolicyDate;
    if (
      previousPolicyDate === null ||
      previousPolicyDate === currentPolicyDate
    ) {
      return;
    }
    void getLiveQuota()
      .then((liveQuotaState) => {
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
      })
      .catch(() => undefined);
  }, [currentPolicyDate, isPreview, state.kind]);

  useEffect(() => {
    const params = new URLSearchParams(window.location.search);
    const theme = params.get("theme");
    const surface = params.get("surface");
    const fontScale = params.get("fontScale");

    document.documentElement.dataset.runtime = isPreview
      ? "preview"
      : "desktop";
    if (theme === "light" || theme === "dark") {
      document.documentElement.dataset.theme = theme;
    } else if (isPreview) {
      delete document.documentElement.dataset.theme;
    }

    if (surface === "opaque") {
      document.documentElement.dataset.surface = "opaque";
    } else if (
      document.documentElement.dataset.platformFallback !== "true"
    ) {
      document.documentElement.dataset.surface = "glass";
    }
    if (fontScale === "2") {
      document.documentElement.dataset.fontScale = "2";
    } else {
      delete document.documentElement.dataset.fontScale;
    }
  }, [isPreview]);

  useEffect(() => {
    document.body.dataset.platformFallbackMessage = translate(
      platformFallbackLocale,
      "surface.opaqueFallback",
    );
  }, [platformFallbackLocale]);

  useEffect(() => {
    if (typeof window.matchMedia !== "function") {
      return;
    }
    const preferences = [
      window.matchMedia("(prefers-reduced-transparency: reduce)"),
      window.matchMedia("(prefers-contrast: more)"),
      window.matchMedia("(forced-colors: active)"),
    ];
    const apply = () => {
      const opaque = preferences.some((preference) => preference.matches);
      if (opaque) {
        document.documentElement.dataset.surface = "opaque";
      } else if (
        document.documentElement.dataset.platformFallback !== "true"
      ) {
        document.documentElement.dataset.surface = "glass";
      }
      if (!isPreview) {
        void setAccessibleSurface(opaque).then((supported) => {
          if (!supported) {
            document.documentElement.dataset.surface = "opaque";
            document.documentElement.dataset.platformFallback = "true";
          }
        }).catch(() => undefined);
      }
    };
    apply();
    for (const preference of preferences) {
      preference.addEventListener("change", apply);
    }
    return () => {
      for (const preference of preferences) {
        preference.removeEventListener("change", apply);
      }
    };
  }, [isPreview]);

  if (state.kind === "loading") {
    return (
      <main class="boot-state" aria-busy="true" role="status">
        {text("正在连接 Rust 核心…", "Connecting to the Rust core…")}
      </main>
    );
  }

  if (state.kind === "error") {
    return (
      <main>
        <p role="alert">
          {text(
            "桌面外壳不可用，请从托盘菜单重试。",
            "The desktop shell is unavailable. Try again from the tray menu.",
          )}
        </p>
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

  const fixture = isPreview
    ? projectLiveFixture(
        state.settings,
        state.liveQuota,
        PREVIEW_NOW_UNIX_MS,
        state.radar,
        resolveInterfaceLocale(
          state.settings.interfaceLocale,
          state.settings.formatLocale,
        ),
        state.settings.formatLocale,
        state.resetCredits,
      )
    : projectLiveFixture(
        state.settings,
        state.liveQuota,
        nowUnixMs,
        state.radar,
        resolveInterfaceLocale(
          state.settings.interfaceLocale,
          navigator.languages[0] || navigator.language || "en",
        ),
        navigator.languages[0] || navigator.language || "en",
        state.resetCredits,
      );

  const replaceAlerts = (alerts: PublicAlertInbox) => {
    setState((current) =>
      current.kind === "ready" ? { ...current, alerts } : current,
    );
  };
  const handleDismissAlert = async (eventId: number) => {
    if (isPreview) {
      replaceAlerts({
        ...state.alerts,
        events: state.alerts.events.filter(
          (event) => event.eventId !== eventId,
        ),
      });
      return;
    }
    replaceAlerts(await dismissAlert(eventId));
  };
  const handleDismissAllAlerts = async () => {
    if (isPreview) {
      replaceAlerts({ ...state.alerts, events: [] });
      return;
    }
    replaceAlerts(await dismissAllAlerts());
  };

  return (
    <I18nProvider preference={state.settings.interfaceLocale}>
      <TrayApp
      fixture={fixture}
      settings={state.settings}
      alerts={state.alerts}
      focusRequest={state.focusRequest}
      externalRefreshing={state.refreshing}
      recoveredFromBackup={state.recoveredFromBackup}
      updateState={state.updateState}
      onWeekDetailChange={
        isPreview ? undefined : setMainWindowExpanded
      }
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
      onDismissAlert={handleDismissAlert}
      onDismissAllAlerts={handleDismissAllAlerts}
      onRequestNotificationPermission={requestSystemNotificationPermission}
      onPickAuthFile={isPreview ? undefined : pickAuthFile}
      onSendTestEmail={sendTestEmail}
      onExportDiagnostics={exportDiagnostics}
      onClearLocalData={clearAllLocalData}
      onReloadSettings={getSettings}
      onCheckForUpdate={async () => {
        const updateState = await requestUpdateCheck();
        setState((current) =>
          current.kind === "ready" ? { ...current, updateState } : current,
        );
        return updateState;
      }}
      onInstallUpdate={async () => {
        const updateState = await installPendingUpdate();
        setState((current) =>
          current.kind === "ready" ? { ...current, updateState } : current,
        );
        return updateState;
      }}
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
    </I18nProvider>
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
  const { text } = useI18n();
  const [busy, setBusy] = useState(false);
  const [actionFailed, setActionFailed] = useState(false);
  const content =
    startup.mode === "unsupported_schema"
      ? {
            eyebrow: text("版本保护", "Version protection"),
            title: text(
              "本地数据来自更新版本",
              "Local data belongs to a newer version",
            ),
            detail: text(
              "QuotaTide 已保持只读，没有降级或覆盖数据。请安装兼容版本后重试。",
              "QuotaTide kept the data read-only and did not downgrade or overwrite it. Install a compatible version and try again.",
            ),
        }
      : startup.mode === "storage_permission_denied"
        ? {
            eyebrow: text("隐私保护", "Privacy protection"),
            title: text(
              "无法保护本地数据目录",
              "The local data directory cannot be secured",
            ),
            detail: text(
              "应用已停止数据库写入。请检查目录所有者与权限，修复后再重试。",
              "Database writes have stopped. Check directory ownership and permissions, then try again.",
            ),
          }
        : {
            eyebrow: text("恢复模式", "Recovery mode"),
            title: text(
              "本地账本需要处理",
              "The local ledger needs attention",
            ),
            detail: text(
              "有效备份已自动尝试。当前没有可安全使用的数据副本，因此没有创建空账本。",
              "Valid backups were tried automatically. No safe copy is currently available, so an empty ledger was not created.",
            ),
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
            aria-label={text("隐藏窗口", "Hide window")}
            onClick={() => void run(onHide)}
          >
            ×
          </button>
        </header>
        <p class="recovery-detail">{content.detail}</p>
        <div class="recovery-note">
          <strong>
            {text(
              "你的 auth.json 不在处理范围内",
              "Your auth.json is outside this operation",
            )}
          </strong>
          <span>
            {text(
              "恢复与清除操作只会作用于 QuotaTide 自己的应用数据。",
              "Recovery and deletion affect only QuotaTide's own app data.",
            )}
          </span>
        </div>
        <div class="recovery-privacy-tools">
          <PrivacyPanel
            onExportDiagnostics={onExportDiagnostics}
            onClearLocalData={onClearLocalData}
          />
        </div>
        {actionFailed ? (
          <p class="settings-error" role="alert">
            {text(
              "操作未完成，请检查系统权限后重试。",
              "The operation did not finish. Check system permissions and try again.",
            )}
          </p>
        ) : null}
        <footer class="recovery-actions">
          <button
            type="button"
            disabled={busy}
            onClick={() => void run(onOpenData)}
          >
            {text("打开数据目录", "Open data directory")}
          </button>
          <button
            type="button"
            class="settings-save"
            disabled={busy}
            onClick={() => void run(onRetry)}
          >
            {busy
              ? text("处理中…", "Working…")
              : text("重试恢复", "Retry recovery")}
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
  interfaceLocale: InterfaceLocale = "zh-CN",
  formatLocale = "zh-CN",
  resetCredits: PublicResetCredits | null = null,
): LedgerFixture {
  if (!account.configured) {
    return {
      tone: "unconfigured",
      pressure: "safe",
      weeklyUsed: "",
      weeklyRemaining: "",
      burnProjection: null,
      resetCredits: null,
      todayAvailable: "",
      todayAvailabilityKind: "unavailable",
      todayLimit: "",
      sourceHealth:
        interfaceLocale === "zh-CN" ? "尚未连接" : "Not connected",
      windowLabel: "",
      lastSuccess:
        interfaceLocale === "zh-CN" ? "尚未同步" : "Not synced yet",
      resetAbsolute: "",
      resetCompact: "",
      resetRelative: "",
      radar: projectRadarFixture(radar, interfaceLocale, formatLocale),
      days: [],
    };
  }
  if (live === null) {
    return {
      tone: "stale",
      pressure: "safe",
      weeklyUsed: "",
      weeklyRemaining: "",
      burnProjection: null,
      resetCredits: null,
      sourceHealth: copy(
        interfaceLocale,
        "Codex 额度 · 等待首次同步",
        "Codex quota · Waiting for first sync",
      ),
      windowLabel: "",
      lastSuccess: copy(
        interfaceLocale,
        "尚未成功同步",
        "No successful sync yet",
      ),
      resetAbsolute: "",
      resetCompact: "",
      resetRelative: "",
      todayAvailable: "",
      todayAvailabilityKind: "unavailable",
      todayLimit: "",
      radar: projectRadarFixture(radar, interfaceLocale, formatLocale),
      days: [],
    };
  }

  const resetMs =
    live.resetsAtUnixS === null ? null : live.resetsAtUnixS * 1000;
  const resetTime = formatResetTime(
    resetMs,
    now,
    interfaceLocale,
    formatLocale,
    account.quotaPolicy.policyTimezone,
  );
  const used = formatMicropoints(live.usedMicropoints, formatLocale);
  const remaining = formatMicropoints(live.remainingMicropoints, formatLocale);
  const sourceHealth = sourceHealthLabel(live, interfaceLocale);
  const days = live.ledgerDays.map((day) =>
    projectLedgerDay(day, interfaceLocale, formatLocale),
  );
  const today = live.ledgerDays.find((day) => day.isToday);
  const todayAvailable = formatMicropoints(
    live.todayAvailableMicropoints,
    formatLocale,
  );
  const todayLimit =
    live.todayLimitMicropoints === null ||
    live.todayBaseMicropoints === null ||
    live.todayCarryMicropoints === null
      ? ""
      : copy(
          interfaceLocale,
          `基础 ${formatMicropoints(live.todayBaseMicropoints, formatLocale)} + 结转 ${formatMicropoints(live.todayCarryMicropoints, formatLocale)} = 实际 ${formatMicropoints(live.todayLimitMicropoints, formatLocale)}`,
          `Base ${formatMicropoints(live.todayBaseMicropoints, formatLocale)} + carry ${formatMicropoints(live.todayCarryMicropoints, formatLocale)} = adjusted ${formatMicropoints(live.todayLimitMicropoints, formatLocale)}`,
        );
  const tone =
    live.sourceStatus !== "fresh"
      ? "stale"
      : today?.status === "exceeded"
        ? "over"
        : today?.status === "warning"
          ? "warning"
          : "fresh";
  const nearestCreditExpiry = resetCredits?.credits
    .filter(
      (credit) =>
        credit.status === "available" &&
        credit.expiresAtUnixS !== null &&
        credit.expiresAtUnixS * 1_000 > now,
    )
    .map((credit) => credit.expiresAtUnixS as number)
    .sort((left, right) => left - right)[0];
  const resetCreditsFixture =
    resetCredits === null
      ? null
      : {
          availableLabel: copy(
            interfaceLocale,
            `可用 ${String(resetCredits.availableCount)} 次`,
            `${String(resetCredits.availableCount)} available`,
          ),
          expiryLabel:
            nearestCreditExpiry === undefined
              ? copy(interfaceLocale, "暂无到期信息", "No expiry information")
              : copy(
                  interfaceLocale,
                  `最近一枚 ${String(Math.max(1, Math.ceil((nearestCreditExpiry * 1_000 - now) / 86_400_000)))} 天后到期`,
                  `Next credit expires in ${String(Math.max(1, Math.ceil((nearestCreditExpiry * 1_000 - now) / 86_400_000)))} days`,
                ),
        };
  const burnProjection =
    live.burnProjection === null
      ? null
      : {
          rate: `${formatMicropoints(live.burnProjection.rateMicropointsPerHour, formatLocale)}/${copy(interfaceLocale, "小时", "h")}`,
          projectedUsage: formatMicropoints(
            live.burnProjection.projectedUsedAtResetMicropoints,
            formatLocale,
          ),
          conclusion:
            live.burnProjection.exhaustsAtUnixS === null
              ? copy(
                  interfaceLocale,
                  `按当前速度，到重置预计使用 ${formatMicropoints(live.burnProjection.projectedUsedAtResetMicropoints, formatLocale)}`,
                  `At this rate, projected to use ${formatMicropoints(live.burnProjection.projectedUsedAtResetMicropoints, formatLocale)} by reset`,
                )
              : copy(
                  interfaceLocale,
                  `预计 ${formatDateTime(live.burnProjection.exhaustsAtUnixS * 1_000, formatLocale)} 触顶，早于重置`,
                  `Expected to hit the limit ${formatDateTime(live.burnProjection.exhaustsAtUnixS * 1_000, formatLocale)}, before reset`,
                ),
        };
  return {
    tone,
    pressure: live.pressure,
    weeklyUsed: used,
    weeklyRemaining: remaining,
    burnProjection,
    resetCredits: resetCreditsFixture,
    sourceHealth,
    windowLabel:
      days.length === 0
        ? live.windowStartsAtUnixS === null || live.windowEndsAtUnixS === null
          ? ""
          : `${formatDate(live.windowStartsAtUnixS * 1000, formatLocale)} ${copy(interfaceLocale, "至", "to")} ${formatDate(live.windowEndsAtUnixS * 1000, formatLocale)}`
        : `${days[0].date} ${copy(interfaceLocale, "至", "to")} ${days.at(-1)?.date ?? ""}`,
    lastSuccess:
      live.lastSuccessAtUnixMs === null
        ? copy(interfaceLocale, "尚未成功同步", "No successful sync yet")
        : `${copy(interfaceLocale, "上次成功", "Last successful sync")} ${formatDateTime(live.lastSuccessAtUnixMs, formatLocale)}`,
    resetAbsolute: resetTime?.absolute ?? "",
    resetCompact: resetTime?.compact ?? "",
    resetRelative: resetTime?.relative ?? "",
    todayAvailable,
    todayAvailabilityKind: live.todayAvailabilityKind,
    todayLimit,
    radar: projectRadarFixture(radar, interfaceLocale, formatLocale),
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

export function projectRadarFixture(
  radar: PublicResetRadar,
  interfaceLocale: InterfaceLocale = "zh-CN",
  formatLocale = "zh-CN",
) {
  const announcement =
    radar.latestAnnouncement === null
      ? null
      : {
          text: localizeRadarText(
            radar.latestAnnouncement.text,
            interfaceLocale,
          ),
          sourceUrl: radar.latestAnnouncement.sourceUrl,
          announcedAt: formatDateTime(
            radar.latestAnnouncement.announcedAtUnixMs,
            formatLocale,
          ),
        };
  const prediction = radar.prediction;
  if (prediction !== null) {
    const health =
      radar.sourceStatus === "fresh"
        ? copy(interfaceLocale, "数据源正常", "Source healthy")
        : radar.sourceStatus === "stale_after_failure"
          ? copy(
              interfaceLocale,
              "数据源暂不可用，显示有效快照",
              "Source unavailable; showing a valid snapshot",
            )
          : copy(
              interfaceLocale,
              "显示仍有效的最后快照",
              "Showing the last valid snapshot",
            );
    return {
      kind: "active" as const,
      chance: prediction.displayChance,
      explanation: localizeRadarText(prediction.explanation, interfaceLocale),
      sourceUrl: prediction.sourceUrl,
      timing: `${copy(interfaceLocale, "预计", "Expected")} ${formatDateTime(prediction.expiresAtUnixMs, formatLocale)}`,
      health,
      announcement,
    };
  }
  const message =
      radar.sourceStatus === "fresh"
      ? copy(
          interfaceLocale,
          "当前无计划重置信号",
          "No scheduled reset signal",
        )
      : radar.lastAttemptAtUnixMs === null
        ? copy(
            interfaceLocale,
            "等待首次雷达同步",
            "Waiting for the first radar sync",
          )
        : copy(
            interfaceLocale,
            "重置数据暂不可用",
            "Reset data is unavailable",
          );
  return {
    kind: "empty" as const,
    message,
    announcement,
  };
}

function localizeRadarText(
  value: string,
  interfaceLocale: InterfaceLocale,
): string {
  if (value === "Explicit Codex quota reset schedule.") {
    return copy(
      interfaceLocale,
      "公开信号明确提到 Codex 额度重置计划。",
      value,
    );
  }
  if (value === "Explicit Codex quota reset announcement.") {
    return copy(
      interfaceLocale,
      "公开信号明确宣布 Codex 额度已经重置。",
      value,
    );
  }
  return value;
}

function projectLedgerDay(
  day: PublicLedgerDay,
  interfaceLocale: InterfaceLocale,
  formatLocale: string,
) {
  const [year, month, date] = day.localDate
    .split("-")
    .map((part) => Number.parseInt(part, 10));
  const naturalDate = new Date(year, month - 1, date);
  const used =
    day.usedMicropoints === null ? null : day.usedMicropoints / 1_000_000;
  const limit = day.limitMicropoints / 1_000_000;
  const suggested =
    day.suggestedLimitMicropoints === null
      ? null
      : day.suggestedLimitMicropoints / 1_000_000;
  return {
    label: day.isToday
      ? copy(interfaceLocale, "今天", "Today")
      : new Intl.DateTimeFormat(formatLocale, { weekday: "short" }).format(
          naturalDate,
        ),
    date: new Intl.DateTimeFormat(formatLocale, {
      month: "2-digit",
      day: "2-digit",
    }).format(naturalDate),
    used,
    limit,
    suggested,
    today: day.isToday,
    status: ledgerStatusLabel(day.status, interfaceLocale),
  };
}

function ledgerStatusLabel(
  status: PublicLedgerDay["status"],
  locale: InterfaceLocale,
): string {
  switch (status) {
    case "unknown":
      return copy(locale, "尚无记录", "No record yet");
    case "normal":
      return copy(locale, "进行中", "In progress");
    case "warning":
      return copy(locale, "接近上限", "Approaching limit");
    case "exceeded":
      return copy(locale, "已达上限", "Limit reached");
    case "finalized":
      return copy(locale, "已封存", "Finalized");
  }
}

function sourceHealthLabel(
  live: PublicLiveQuota,
  locale: InterfaceLocale,
): string {
  switch (live.sourceStatus) {
    case "fresh":
      return copy(locale, "Codex 额度 · 正常", "Codex quota · Healthy");
    case "stale_after_failure":
      return copy(
        locale,
        `Codex 额度 · 连续 ${live.consecutiveFailures.toString()} 次失败（${usageErrorLabel(live.publicError, locale)}）`,
        `Codex quota · ${live.consecutiveFailures.toString()} consecutive failures (${usageErrorLabel(live.publicError, locale)})`,
      );
    case "stale_by_age":
      return copy(
        locale,
        "Codex 额度 · 数据超过 90 分钟",
        "Codex quota · Over 90 minutes old",
      );
    case "unavailable":
      return live.consecutiveFailures > 0
        ? copy(
            locale,
            `Codex 额度 · 首次同步失败（${usageErrorLabel(live.publicError, locale)}）`,
            `Codex quota · First sync failed (${usageErrorLabel(live.publicError, locale)})`,
          )
        : copy(
            locale,
            "Codex 额度 · 等待首次同步",
            "Codex quota · Waiting for first sync",
          );
  }
}

function usageErrorLabel(
  error: UsageSourceErrorCode | null,
  locale: InterfaceLocale,
): string {
  switch (error) {
    case "auth_path_unavailable":
      return copy(locale, "账号文件不可用", "Account file unavailable");
    case "authentication_stale":
      return copy(locale, "登录已失效", "Sign-in expired");
    case "permission_denied":
      return copy(locale, "访问被拒绝", "Access denied");
    case "rate_limited":
      return copy(locale, "请求过于频繁", "Rate limited");
    case "timeout":
      return copy(locale, "请求超时", "Request timed out");
    case "upstream_unavailable":
      return copy(locale, "Codex 服务暂不可用", "Codex service unavailable");
    case "response_too_large":
    case "invalid_json":
    case "contract_violation":
    case "weekly_window_unavailable":
      return copy(
        locale,
        "额度响应暂不可识别",
        "Quota response not recognized",
      );
    case null:
      return copy(locale, "未知原因", "Unknown reason");
  }
}

function formatMicropoints(value: number | null, formatLocale: string): string {
  return formatPercent(value, formatLocale);
}

function formatDate(value: number, formatLocale: string): string {
  return new Intl.DateTimeFormat(formatLocale, {
    month: "2-digit",
    day: "2-digit",
  }).format(value);
}

function formatDateTime(value: number, formatLocale: string): string {
  return new Intl.DateTimeFormat(formatLocale, {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(value);
}

function copy(
  locale: InterfaceLocale,
  zhCn: string,
  english: string,
): string {
  return locale === "zh-CN" ? zhCn : english;
}
