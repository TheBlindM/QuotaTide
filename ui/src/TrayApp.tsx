import { useCallback, useEffect, useRef, useState } from "preact/hooks";

import type { AlertChannel } from "./bindings/AlertChannel";
import type { AlertEventKind } from "./bindings/AlertEventKind";
import type { AlertPreferenceDraft } from "./bindings/AlertPreferenceDraft";
import type { InterfaceLocalePreference } from "./bindings/InterfaceLocalePreference";
import type { NotificationPermissionStatus } from "./bindings/NotificationPermissionStatus";
import type { PublicAlertInbox } from "./bindings/PublicAlertInbox";
import type { PublicSettings } from "./bindings/PublicSettings";
import type { SettingsDraft } from "./bindings/SettingsDraft";
import type { StoryTheme } from "./bindings/StoryTheme";
import type { TrayDisplayMode } from "./bindings/TrayDisplayMode";
import {
  applyColorTheme,
  readInitialColorTheme,
  rememberColorTheme,
  ThemeToggle,
  type ColorTheme,
} from "./color-theme";
import type { PublicUpdateState } from "./api/updater";
import type { NotificationActivation } from "./api/alerts";
import { useI18n } from "./i18n-context";
import { StoryThemePicker } from "./story";
import { WeeklyLedger, type LedgerFixture } from "./WeeklyLedger";

const alertEvents: ReadonlyArray<{
  kind: AlertEventKind;
  label: string;
  detail: string;
  enLabel: string;
  enDetail: string;
}> = [
  {
    kind: "daily_80",
    label: "每日额度达到 80%",
    detail: "今日实际使用接近动态上限",
    enLabel: "Daily quota reaches 80%",
    enDetail: "Today's usage is approaching its adjusted limit",
  },
  {
    kind: "daily_100",
    label: "每日额度达到 100%",
    detail: "今日实际使用达到动态上限",
    enLabel: "Daily quota reaches 100%",
    enDetail: "Today's usage reaches its adjusted limit",
  },
  {
    kind: "weekly_remaining_20",
    label: "周额度剩余 20%",
    detail: "当前七日窗口进入注意区间",
    enLabel: "20% weekly quota remains",
    enDetail: "The current seven-day window enters the caution range",
  },
  {
    kind: "weekly_remaining_10",
    label: "周额度剩余 10%",
    detail: "当前七日窗口接近耗尽",
    enLabel: "10% weekly quota remains",
    enDetail: "The current seven-day window is nearly exhausted",
  },
  {
    kind: "radar_chance_70",
    label: "重置预测置信度达到 70%",
    detail: "第三方公开信号预测近期可能重置",
    enLabel: "Reset prediction confidence reaches 70%",
    enDetail: "A third-party public signal predicts a near-term reset",
  },
  {
    kind: "quota_reset_confirmed",
    label: "额度重置已确认",
    detail: "本机连续观测确认新额度窗口",
    enLabel: "Quota reset confirmed",
    enDetail: "Consecutive local observations confirm a new quota window",
  },
  {
    kind: "source_failures_3",
    label: "连续 3 次采集失败",
    detail: "Codex 或 Reset Radar 暂时不可用",
    enLabel: "Three consecutive refresh failures",
    enDetail: "Codex or Reset Radar is temporarily unavailable",
  },
];

const defaultAlertPreferences: AlertPreferenceDraft[] = alertEvents.flatMap(
  ({ kind }) => [
    { eventKind: kind, channel: "system", enabled: true },
    { eventKind: kind, channel: "email", enabled: false },
  ],
);

const unconfiguredSettings: PublicSettings = {
  settingsRevision: 0,
  configured: false,
  pathSummary: null,
  accountLabel: null,
  notificationPermissionStatus: "unknown",
  quotaPolicy: {
    policyRevision: 1,
    policyTimezone: "Asia/Shanghai",
    carryWorkdaysEnabled: true,
    baseMicropoints: [
      16_000_000, 16_000_000, 16_000_000, 16_000_000, 16_000_000,
      10_000_000, 10_000_000,
    ],
  },
  alertPreferences: defaultAlertPreferences,
  autostartEnabled: false,
  autoUpdateEnabled: true,
  trayDisplayMode: "wave",
  storyTheme: "rising_water",
  interfaceLocale: "system",
  formatLocale: "zh-CN",
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
};

export function PrivacyPanel({
  onExportDiagnostics,
  onClearLocalData,
}: {
  onExportDiagnostics?: () => Promise<boolean>;
  onClearLocalData?: () => Promise<void>;
}) {
  const { locale, text } = useI18n();
  const clearConfirmationPhrase = locale === "zh-CN" ? "清除" : "DELETE";
  const [showExportSummary, setShowExportSummary] = useState(false);
  const [exportState, setExportState] = useState<
    "idle" | "exporting" | "saved" | "cancelled" | "error"
  >("idle");
  const [showClearConfirmation, setShowClearConfirmation] = useState(false);
  const [clearPhrase, setClearPhrase] = useState("");
  const [clearState, setClearState] = useState<"idle" | "clearing" | "error">(
    "idle",
  );

  return (
    <section aria-labelledby="privacy-settings">
      <div class="settings-section__heading">
        <span>{text("本机数据", "Local data")}</span>
        <h2 id="privacy-settings">
          {text("诊断与清除", "Diagnostics and deletion")}
        </h2>
      </div>
      <p class="settings-description">
        {text(
          "数据仅保存在当前用户目录。auth.json 始终只读，且不会被清除。",
          "Data stays in the current user's directory. auth.json is always read-only and is never deleted.",
        )}
      </p>
      <div class="privacy-tool">
        <div>
          <strong>
            {text("导出脱敏诊断", "Export redacted diagnostics")}
          </strong>
          <small>
            {text(
              "用于排查启动、同步或通知问题，不会上传任何内容。",
              "Helps troubleshoot startup, sync, or notification issues. Nothing is uploaded.",
            )}
          </small>
        </div>
        {!showExportSummary ? (
          <button
            type="button"
            disabled={!onExportDiagnostics}
            onClick={() => {
              setShowExportSummary(true);
            }}
          >
            {text("查看内容", "Review contents")}
          </button>
        ) : (
          <div class="privacy-review">
            <p>{text("将包含：", "Includes:")}</p>
            <ul>
              <li>
                {text(
                  "应用、系统与数据库完整性信息",
                  "App, system, and database integrity information",
                )}
              </li>
              <li>
                {text(
                  "脱敏设置、当前额度窗口和来源状态",
                  "Redacted settings, current quota window, and source health",
                )}
              </li>
              <li>
                {text(
                  "最多 5 MiB 的结构化安全日志",
                  "Up to 5 MiB of structured safe logs",
                )}
              </li>
            </ul>
            <p>
              {text(
                "不会包含 Token、账号 ID、邮箱、SMTP 主机、auth 路径或数据库。",
                "Never includes tokens, account IDs, email addresses, SMTP hosts, auth paths, or the database.",
              )}
            </p>
            <button
              type="button"
              class="settings-save"
              disabled={!onExportDiagnostics || exportState === "exporting"}
              onClick={() => {
                if (!onExportDiagnostics) {
                  return;
                }
                setExportState("exporting");
                void onExportDiagnostics()
                  .then((saved) => {
                    setExportState(saved ? "saved" : "cancelled");
                  })
                  .catch(() => {
                    setExportState("error");
                  });
              }}
            >
              {exportState === "exporting"
                ? text("正在准备…", "Preparing…")
                : text("选择保存位置", "Choose save location")}
            </button>
            <span class="privacy-tool__status" role="status">
              {
                {
                  idle: "",
                  exporting: text(
                    "正在生成严格脱敏的 ZIP…",
                    "Generating a strictly redacted ZIP…",
                  ),
                  saved: text("诊断 ZIP 已保存", "Diagnostic ZIP saved"),
                  cancelled: text("已取消导出", "Export cancelled"),
                  error: text(
                    "导出失败，请检查目录权限",
                    "Export failed. Check directory permissions.",
                  ),
                }[exportState]
              }
            </span>
          </div>
        )}
      </div>
      <div class="privacy-tool privacy-tool--danger">
        <div>
          <strong>
            {text(
              "清除全部 QuotaTide 本地数据",
              "Delete all local QuotaTide data",
            )}
          </strong>
          <small>
            {text(
              "删除账本、设置、提醒、备份、日志和系统钥匙串密码。",
              "Deletes ledgers, settings, alerts, backups, logs, and the system-vault password.",
            )}
          </small>
        </div>
        {!showClearConfirmation ? (
          <button
            type="button"
            disabled={!onClearLocalData}
            onClick={() => {
              setShowClearConfirmation(true);
            }}
          >
            {text("清除…", "Delete…")}
          </button>
        ) : (
          <div class="privacy-review">
            <p>
              {text("此操作不可撤销。请输入", "This cannot be undone. Enter")}{" "}
              <strong>{clearConfirmationPhrase}</strong>{" "}
              {text("以进行第二次确认。", "to confirm a second time.")}
            </p>
            <input
              type="text"
              aria-label={text("输入清除以确认", "Enter DELETE to confirm")}
              value={clearPhrase}
              autocomplete="off"
              onInput={(event) => {
                setClearPhrase(event.currentTarget.value);
                setClearState("idle");
              }}
            />
            <div class="privacy-confirm-actions">
              <button
                type="button"
                onClick={() => {
                  setShowClearConfirmation(false);
                  setClearPhrase("");
                }}
              >
                {text("取消", "Cancel")}
              </button>
              <button
                type="button"
                class="danger-button"
                disabled={
                  clearPhrase !== clearConfirmationPhrase ||
                  !onClearLocalData ||
                  clearState === "clearing"
                }
                onClick={() => {
                  if (!onClearLocalData) {
                    return;
                  }
                  setClearState("clearing");
                  void onClearLocalData().catch(() => {
                    setClearState("error");
                  });
                }}
              >
                {clearState === "clearing"
                  ? text("正在清除…", "Deleting…")
                  : text(
                      "永久清除并重新启动",
                      "Delete permanently and restart",
                    )}
              </button>
            </div>
            {clearState === "error" ? (
              <p class="settings-error" role="alert">
                {text(
                  "未能删除系统钥匙串或自动启动项。本地数据尚未清除，请重试。",
                  "Could not remove the system-vault credential or login item. Local data was not deleted; try again.",
                )}
              </p>
            ) : null}
          </div>
        )}
      </div>
    </section>
  );
}

type TrayAppProps = {
  fixture: LedgerFixture;
  settings?: PublicSettings;
  alerts?: PublicAlertInbox | null;
  focusRequest?: NotificationActivation | null;
  externalRefreshing?: boolean;
  recoveredFromBackup?: boolean;
  updateState?: PublicUpdateState;
  onHide: () => void;
  onRefresh: () => unknown;
  onDismissAlert?: (eventId: number) => unknown;
  onDismissAllAlerts?: () => unknown;
  onWeekDetailChange?: (expanded: boolean) => unknown;
  onPickAuthFile?: () => Promise<string | null>;
  onRequestNotificationPermission?: () => Promise<NotificationPermissionStatus>;
  onSendTestEmail?: () => Promise<number>;
  onExportDiagnostics?: () => Promise<boolean>;
  onClearLocalData?: () => Promise<void>;
  onSaveSettings?: (draft: SettingsDraft) => Promise<PublicSettings>;
  onReloadSettings?: () => Promise<PublicSettings>;
  onCheckForUpdate?: () => Promise<PublicUpdateState>;
  onInstallUpdate?: () => Promise<PublicUpdateState>;
};

type SettingsTab = "account" | "quota" | "alerts" | "privacy";

function SettingsView({
  settings,
  theme,
  onToggleTheme,
  onBack,
  onPickAuthFile,
  onRequestNotificationPermission,
  onSendTestEmail,
  onExportDiagnostics,
  onClearLocalData,
  updateState,
  onCheckForUpdate,
  onInstallUpdate,
  onSave,
}: {
  settings: PublicSettings;
  theme: ColorTheme;
  onToggleTheme: () => void;
  onBack: () => void;
  onPickAuthFile?: () => Promise<string | null>;
  onRequestNotificationPermission?: () => Promise<void>;
  onSendTestEmail?: () => Promise<number>;
  onExportDiagnostics?: () => Promise<boolean>;
  onClearLocalData?: () => Promise<void>;
  updateState?: PublicUpdateState;
  onCheckForUpdate?: () => Promise<PublicUpdateState>;
  onInstallUpdate?: () => Promise<PublicUpdateState>;
  onSave: (draft: SettingsDraft) => Promise<void>;
}) {
  const { formatLocale, locale, text, t } = useI18n();
  const titleRef = useRef<HTMLHeadingElement>(null);
  const tabRefs = useRef<Array<HTMLButtonElement | null>>([]);
  const [activeTab, setActiveTab] = useState<SettingsTab>("account");
  const [authPath, setAuthPath] = useState("");
  const [dailyLimits, setDailyLimits] = useState(() =>
    settings.quotaPolicy.baseMicropoints.map((value) => value / 1_000_000),
  );
  const [policyTimezone, setPolicyTimezone] = useState(
    settings.quotaPolicy.policyTimezone,
  );
  const [carryEnabled, setCarryEnabled] = useState(
    settings.quotaPolicy.carryWorkdaysEnabled,
  );
  const [alertPreferences, setAlertPreferences] = useState<
    AlertPreferenceDraft[]
  >(() => settings.alertPreferences.map((preference) => ({ ...preference })));
  const [autostartEnabled, setAutostartEnabled] = useState(
    settings.autostartEnabled,
  );
  const [autoUpdateEnabled, setAutoUpdateEnabled] = useState(
    settings.autoUpdateEnabled,
  );
  const [trayDisplayMode, setTrayDisplayMode] = useState<TrayDisplayMode>(
    settings.trayDisplayMode,
  );
  const [storyTheme, setStoryTheme] = useState<StoryTheme>(
    settings.storyTheme,
  );
  const [confirmUpdate, setConfirmUpdate] = useState(false);
  const [interfaceLocale, setInterfaceLocale] =
    useState<InterfaceLocalePreference>(settings.interfaceLocale);
  const [smtpEnabled, setSmtpEnabled] = useState(settings.smtp.enabled);
  const [smtpHost, setSmtpHost] = useState(settings.smtp.host);
  const [smtpPort, setSmtpPort] = useState(String(settings.smtp.port));
  const [smtpTlsMode, setSmtpTlsMode] = useState(settings.smtp.tlsMode);
  const [smtpUsername, setSmtpUsername] = useState(settings.smtp.username);
  const [smtpFromAddress, setSmtpFromAddress] = useState(
    settings.smtp.fromAddress,
  );
  const [smtpFromName, setSmtpFromName] = useState(settings.smtp.fromName);
  const [smtpRecipients, setSmtpRecipients] = useState(() =>
    settings.smtp.recipients.map((recipient) => ({ ...recipient })),
  );
  const [smtpPassword, setSmtpPassword] = useState("");
  const [deleteSmtpPassword, setDeleteSmtpPassword] = useState(false);
  const [testEmailState, setTestEmailState] = useState<
    "idle" | "sending" | "sent" | "error"
  >("idle");
  const [testEmailCount, setTestEmailCount] = useState(0);
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState(false);
  const [pickingAuthFile, setPickingAuthFile] = useState(false);
  const [authPickError, setAuthPickError] = useState(false);
  const [discardRequested, setDiscardRequested] = useState(false);

  useEffect(() => {
    setAuthPath("");
    setDailyLimits(
      settings.quotaPolicy.baseMicropoints.map((value) => value / 1_000_000),
    );
    setPolicyTimezone(settings.quotaPolicy.policyTimezone);
    setCarryEnabled(settings.quotaPolicy.carryWorkdaysEnabled);
    setAlertPreferences(
      settings.alertPreferences.map((preference) => ({ ...preference })),
    );
    setAutostartEnabled(settings.autostartEnabled);
    setAutoUpdateEnabled(settings.autoUpdateEnabled);
    setTrayDisplayMode(settings.trayDisplayMode);
    setStoryTheme(settings.storyTheme);
    setInterfaceLocale(settings.interfaceLocale);
    setSmtpEnabled(settings.smtp.enabled);
    setSmtpHost(settings.smtp.host);
    setSmtpPort(String(settings.smtp.port));
    setSmtpTlsMode(settings.smtp.tlsMode);
    setSmtpUsername(settings.smtp.username);
    setSmtpFromAddress(settings.smtp.fromAddress);
    setSmtpFromName(settings.smtp.fromName);
    setSmtpRecipients(
      settings.smtp.recipients.map((recipient) => ({ ...recipient })),
    );
    setSmtpPassword("");
    setDeleteSmtpPassword(false);
    setTestEmailState("idle");
    setAuthPickError(false);
    setDiscardRequested(false);
  }, [settings.settingsRevision]);

  useEffect(() => {
    titleRef.current?.focus();
  }, []);

  const total = dailyLimits.reduce((sum, value) => sum + value, 0);
  const policyValid =
    dailyLimits.length === 7 &&
    dailyLimits.every((value) => Number.isFinite(value) && value >= 0) &&
    total <= 100 &&
    policyTimezone.trim().length > 0;
  const parsedSmtpPort = Number(smtpPort);
  const smtpValid =
    !smtpEnabled ||
    (smtpHost.trim().length > 0 &&
      Number.isInteger(parsedSmtpPort) &&
      parsedSmtpPort > 0 &&
      parsedSmtpPort <= 65_535 &&
      smtpUsername.trim().length > 0 &&
      smtpFromAddress.includes("@") &&
      smtpRecipients.some(
        (recipient) => recipient.enabled && recipient.address.includes("@"),
      ));
  const dailyMicropoints = dailyLimits.map((value) =>
    Math.round(value * 1_000_000),
  );
  const hasChanges =
    authPath.trim().length > 0 ||
    dailyMicropoints.some(
      (value, index) => value !== settings.quotaPolicy.baseMicropoints[index],
    ) ||
    policyTimezone.trim() !== settings.quotaPolicy.policyTimezone ||
    carryEnabled !== settings.quotaPolicy.carryWorkdaysEnabled ||
    JSON.stringify(alertPreferences) !==
      JSON.stringify(settings.alertPreferences) ||
    autostartEnabled !== settings.autostartEnabled ||
    autoUpdateEnabled !== settings.autoUpdateEnabled ||
    trayDisplayMode !== settings.trayDisplayMode ||
    storyTheme !== settings.storyTheme ||
    interfaceLocale !== settings.interfaceLocale ||
    smtpEnabled !== settings.smtp.enabled ||
    smtpHost.trim() !== settings.smtp.host ||
    smtpPort !== String(settings.smtp.port) ||
    smtpTlsMode !== settings.smtp.tlsMode ||
    smtpUsername.trim() !== settings.smtp.username ||
    smtpFromAddress.trim() !== settings.smtp.fromAddress ||
    smtpFromName.trim() !== settings.smtp.fromName ||
    JSON.stringify(
      smtpRecipients.map((recipient) => ({
        address: recipient.address.trim(),
        enabled: recipient.enabled,
      })),
    ) !== JSON.stringify(settings.smtp.recipients) ||
    smtpPassword.length > 0 ||
    deleteSmtpPassword;
  const enabledAlertCount = alertPreferences.filter(
    (preference) => preference.enabled,
  ).length;
  const tabSummaries: Record<SettingsTab, string> = {
    account: settings.configured
      ? text("已连接", "Connected")
      : text("需配置", "Setup needed"),
    quota: Number.isFinite(total) ? `${total.toFixed(0)}%` : "—",
    alerts: text(
      `${String(enabledAlertCount)} 项开启`,
      `${String(enabledAlertCount)} active`,
    ),
    privacy: text("仅本机", "On device"),
  };

  const setAlertPreference = (
    eventKind: AlertEventKind,
    channel: AlertChannel,
    enabled: boolean,
  ) => {
    setAlertPreferences((current) =>
      current.map((preference) =>
        preference.eventKind === eventKind && preference.channel === channel
          ? { ...preference, enabled }
          : preference,
      ),
    );
    setSaveError(false);
  };

  const save = () => {
    if (saving || !hasChanges) {
      return;
    }
    if (!policyValid) {
      setActiveTab("quota");
      return;
    }
    if (!smtpValid) {
      setActiveTab("alerts");
      return;
    }
    setSaving(true);
    setSaveError(false);
    void onSave({
      expectedSettingsRevision: settings.settingsRevision,
      authPath: authPath.trim() || null,
      quotaPolicy: {
        policyTimezone: policyTimezone.trim(),
        carryWorkdaysEnabled: carryEnabled,
        baseMicropoints: dailyMicropoints,
      },
      alertPreferences,
      autostartEnabled,
      autoUpdateEnabled,
      trayDisplayMode,
      storyTheme,
      interfaceLocale,
      formatLocale,
      smtp: {
        enabled: smtpEnabled,
        host: smtpHost.trim(),
        port: parsedSmtpPort,
        tlsMode: smtpTlsMode,
        username: smtpUsername.trim(),
        fromAddress: smtpFromAddress.trim(),
        fromName: smtpFromName.trim(),
        recipients: smtpRecipients.map((recipient) => ({
          address: recipient.address.trim(),
          enabled: recipient.enabled,
        })),
      },
      smtpPassword: deleteSmtpPassword
        ? "delete"
        : smtpPassword.length > 0
          ? { set: smtpPassword }
          : "keep",
    })
      .then(() => {
        setSmtpPassword("");
        setDeleteSmtpPassword(false);
      })
      .catch(() => {
        setSaveError(true);
      })
      .finally(() => {
        setSaving(false);
      });
  };

  const requestBack = () => {
    if (hasChanges && !discardRequested) {
      setDiscardRequested(true);
      return;
    }
    onBack();
  };

  useEffect(() => {
    if (!hasChanges) {
      setDiscardRequested(false);
      return;
    }
    const protectUnsavedChanges = (event: KeyboardEvent) => {
      if (event.key !== "Escape") {
        return;
      }
      event.preventDefault();
      event.stopImmediatePropagation();
      if (discardRequested) {
        onBack();
      } else {
        setDiscardRequested(true);
      }
    };
    window.addEventListener("keydown", protectUnsavedChanges, true);
    return () => {
      window.removeEventListener("keydown", protectUnsavedChanges, true);
    };
  }, [discardRequested, hasChanges, onBack]);

  return (
    <article class="settings-view">
      <header class="settings-header">
        <button
          type="button"
          aria-label={text("返回", "Back")}
          onClick={requestBack}
        >
          ←
        </button>
        <div>
          <h1 ref={titleRef} tabIndex={-1}>{text("设置", "Settings")}</h1>
          <p>{text("所有更改将作为一个版本保存", "All changes are saved as one revision")}</p>
        </div>
        <ThemeToggle theme={theme} onToggle={onToggleTheme} />
      </header>

      <main class="settings-content">
        <div
          class="settings-tabs"
          role="tablist"
          aria-label={text("设置分类", "Settings sections")}
          onKeyDown={(event) => {
            const tabs = ["account", "quota", "alerts", "privacy"] as const;
            const current = tabs.indexOf(activeTab);
            const next =
              event.key === "ArrowRight"
                ? (current + 1) % tabs.length
                : event.key === "ArrowLeft"
                  ? (current - 1 + tabs.length) % tabs.length
                  : event.key === "Home"
                    ? 0
                    : event.key === "End"
                      ? tabs.length - 1
                      : null;
            if (next === null) {
              return;
            }
            event.preventDefault();
            setActiveTab(tabs[next]);
            tabRefs.current[next]?.focus();
          }}
        >
          {(
            [
              ["account", text("账号", "Account")],
              ["quota", text("额度", "Quota")],
              ["alerts", text("提醒", "Alerts")],
              ["privacy", text("隐私", "Privacy")],
            ] as const
          ).map(([tab, label]) => (
            <button
              key={tab}
              type="button"
              role="tab"
              ref={(node) => {
                tabRefs.current[["account", "quota", "alerts", "privacy"].indexOf(tab)] = node;
              }}
              id={`settings-tab-${tab}`}
              aria-controls={`settings-panel-${tab}`}
              aria-label={label}
              aria-selected={activeTab === tab}
              data-invalid={
                (tab === "quota" && !policyValid) ||
                (tab === "alerts" && !smtpValid)
                  ? "true"
                  : undefined
              }
              tabIndex={activeTab === tab ? 0 : -1}
              onClick={() => {
                setActiveTab(tab);
              }}
            >
              <span>{label}</span>
              <small>{tabSummaries[tab]}</small>
            </button>
          ))}
        </div>

        {activeTab === "account" ? (
          <section
            id="settings-panel-account"
            role="tabpanel"
            aria-labelledby="settings-tab-account"
          >
            <div class="settings-section__heading">
              <span>{text("数据源", "Data source")}</span>
              <h2 id="account-settings">
                {text("当前 Codex 账号", "Current Codex account")}
              </h2>
            </div>
            <div class="account-status" aria-live="polite">
              <span>
                {settings.configured
                  ? settings.accountLabel
                  : text("尚未配置 Codex 账号", "No Codex account configured")}
              </span>
              {settings.pathSummary ? <strong>{settings.pathSummary}</strong> : null}
            </div>
            <div class="auth-path-field">
              <label for="settings-auth-path">
                {text("auth.json 路径", "auth.json path")}
              </label>
              <div class="auth-path-control">
                <input
                  id="settings-auth-path"
                  type="text"
                  aria-label={text("auth.json 路径", "auth.json path")}
                  value={authPath}
                  placeholder={
                    settings.configured
                      ? text(
                          "留空以保留当前文件",
                          "Leave blank to keep the current file",
                        )
                      : "/Users/name/.codex/auth.json"
                  }
                  autocomplete="off"
                  spellcheck={false}
                  onInput={(event) => {
                    setAuthPath(event.currentTarget.value);
                    setAuthPickError(false);
                    setSaveError(false);
                  }}
                />
                <button
                  type="button"
                  disabled={!onPickAuthFile || pickingAuthFile}
                  onClick={() => {
                    if (!onPickAuthFile) {
                      return;
                    }
                    setPickingAuthFile(true);
                    setAuthPickError(false);
                    void onPickAuthFile()
                      .then((path) => {
                        if (path) {
                          setAuthPath(path);
                          setSaveError(false);
                        }
                      })
                      .catch(() => {
                        setAuthPickError(true);
                      })
                      .finally(() => {
                        setPickingAuthFile(false);
                      });
                  }}
                >
                  {pickingAuthFile
                    ? text("选择中…", "Choosing…")
                    : text("浏览…", "Browse…")}
                </button>
              </div>
              {authPickError ? (
                <small class="field-error" role="alert">
                  {text(
                    "无法打开文件选择器，也可以直接粘贴路径。",
                    "The file picker could not open. You can paste the path instead.",
                  )}
                </small>
              ) : null}
            </div>
            <p class="privacy-note">
              {text(
                "只读访问。QuotaTide 不会修改 auth.json，也不会把路径或令牌发给网页。",
                "Read-only access. QuotaTide never modifies auth.json or sends its path or tokens to a website.",
              )}
            </p>
            <label class="settings-row settings-row--separated">
              <span>
                <strong>{t("settings.language")}</strong>
                <small>{t("settings.languageHelp")}</small>
              </span>
              <select
                aria-label={t("settings.language")}
                value={interfaceLocale}
                onChange={(event) => {
                  setInterfaceLocale(
                    event.currentTarget.value as InterfaceLocalePreference,
                  );
                  setSaveError(false);
                }}
              >
                <option value="system">{t("language.system")}</option>
                <option value="zh-CN">{t("language.zh-CN")}</option>
                <option value="en">{t("language.en")}</option>
              </select>
            </label>
            <label class="settings-row settings-row--separated">
              <span>
                <strong>{text("登录后自动启动", "Launch at login")}</strong>
                <small>
                  {text(
                    "仅在当前 macOS 或 Windows 用户下运行",
                    "Runs only for the current macOS or Windows user",
                  )}
                </small>
              </span>
              <input
                aria-label={text("登录后自动启动", "Launch at login")}
                type="checkbox"
                checked={autostartEnabled}
                onChange={(event) => {
                  setAutostartEnabled(event.currentTarget.checked);
                  setSaveError(false);
                }}
              />
            </label>
            <div class="settings-row settings-row--separated story-theme-setting">
              <span>
                <strong>{text("故事主题", "Story theme")}</strong>
                <small>
                  {text(
                    "只改变额度故事表现，不影响额度计算、提醒或数据采集",
                    "Changes only the quota story; calculations, alerts, and data collection stay the same",
                  )}
                </small>
              </span>
              <StoryThemePicker
                value={storyTheme}
                onChange={(themeId) => {
                  setStoryTheme(themeId);
                  setSaveError(false);
                }}
              />
            </div>
            <label class="settings-row settings-row--separated">
              <span>
                <strong>{text("任务栏显示", "Menu bar display")}</strong>
                <small>
                  {text(
                    "圆环水位始终显示；文字仅在系统支持时追加",
                    "The animated water ring is always shown; text is added only when supported",
                  )}
                </small>
              </span>
              <select
                aria-label={text("任务栏显示", "Menu bar display")}
                value={trayDisplayMode}
                onChange={(event) => {
                  setTrayDisplayMode(
                    event.currentTarget.value as TrayDisplayMode,
                  );
                  setSaveError(false);
                }}
              >
                <option value="wave">
                  {text("仅动态圆环", "Animated ring only")}
                </option>
                <option value="wave_weekly_remaining">
                  {text("圆环 + 周剩余", "Ring + weekly remaining")}
                </option>
                <option value="wave_reset_countdown">
                  {text("圆环 + 重置倒计时", "Ring + reset countdown")}
                </option>
              </select>
            </label>
            <div class="update-settings settings-row--separated">
              <div class="settings-section__heading">
                <span>{text("关于与更新", "About and updates")}</span>
                <h2>
                  QuotaTide {updateState?.currentVersion ?? "0.2.0"}
                </h2>
                <small>{text("作者 TheBlind", "By TheBlind")}</small>
              </div>
              <label class="settings-row">
                <span>
                  <strong>{text("自动检查更新", "Check for updates automatically")}</strong>
                  <small>
                    {text(
                      "启动 60 秒后检查，之后每 24 小时最多一次",
                      "Checks after 60 seconds, then at most once every 24 hours",
                    )}
                  </small>
                </span>
                <input
                  aria-label={text("自动检查更新", "Check for updates automatically")}
                  type="checkbox"
                  checked={autoUpdateEnabled}
                  onChange={(event) => {
                    setAutoUpdateEnabled(event.currentTarget.checked);
                    setSaveError(false);
                  }}
                />
              </label>
              <div class="update-actions">
                <p role="status">
                  {updateState?.status === "checking"
                    ? text("正在检查更新…", "Checking for updates…")
                    : updateState?.status === "up_to_date"
                      ? text("已是最新版本", "QuotaTide is up to date")
                      : updateState?.status === "available"
                        ? `${text("发现新版本", "Update available")} ${updateState.availableVersion ?? ""}`
                        : updateState?.status === "installing"
                          ? text("正在验证并安装…", "Verifying and installing…")
                          : updateState?.status === "error"
                            ? text(
                                "更新检查失败；当前版本不受影响",
                                "Update check failed; the current version is unaffected",
                              )
                            : text(
                                "尚未检查更新",
                                "Updates have not been checked yet",
                              )}
                </p>
                <button
                  type="button"
                  disabled={
                    !onCheckForUpdate ||
                    updateState?.status === "checking" ||
                    updateState?.status === "installing"
                  }
                  onClick={() => {
                    setConfirmUpdate(false);
                    void onCheckForUpdate?.();
                  }}
                >
                  {text("手动检查", "Check now")}
                </button>
              </div>
              {updateState?.status === "available" ? (
                <div class="update-available" role="region" aria-label={text("可用更新", "Available update")}>
                  {updateState.notes ? <p>{updateState.notes}</p> : null}
                  {confirmUpdate ? (
                    <div class="update-confirm">
                      <p>
                        {text(
                          "将下载已签名更新、安装并重新启动 QuotaTide。现在继续？",
                          "The signed update will be downloaded, installed, and QuotaTide will restart. Continue?",
                        )}
                      </p>
                      <button
                        type="button"
                        onClick={() => {
                          setConfirmUpdate(false);
                        }}
                      >
                        {text("取消", "Cancel")}
                      </button>
                      <button
                        type="button"
                        class="primary-update"
                        onClick={() => {
                          void onInstallUpdate?.();
                        }}
                      >
                        {text("确认安装", "Install update")}
                      </button>
                    </div>
                  ) : (
                    <button
                      type="button"
                      class="primary-update"
                      disabled={!onInstallUpdate}
                      onClick={() => {
                        setConfirmUpdate(true);
                      }}
                    >
                      {text("安装并重新启动", "Install and restart")}
                    </button>
                  )}
                </div>
              ) : null}
              <p class="privacy-note">
                {text(
                  "当前为未签名开源预览版；系统可能显示未知开发者。更新仍必须通过 QuotaTide 签名验证。",
                  "This is an unsigned open-source preview; your system may show an unknown-developer warning. Updates must still pass QuotaTide signature verification.",
                )}
              </p>
            </div>
          </section>
        ) : activeTab === "quota" ? (
          <section
            id="settings-panel-quota"
            role="tabpanel"
            aria-labelledby="settings-tab-quota"
          >
            <div class="settings-section__heading">
              <span>{text("七日模板", "Seven-day template")}</span>
              <h2 id="quota-settings">
                {text("每日基础额度", "Daily base quota")}
              </h2>
            </div>
            <p class="settings-description">
              {text(
                "七天合计不超过 100%。未用完的工作日额度可平分给同一周后续工作日。",
                "The seven-day total cannot exceed 100%. Unused workday quota can be distributed evenly across later workdays in the same window.",
              )}
            </p>
            <div class="quota-day-grid">
              {[
                ["周一", "Mon"],
                ["周二", "Tue"],
                ["周三", "Wed"],
                ["周四", "Thu"],
                ["周五", "Fri"],
                ["周六", "Sat"],
                ["周日", "Sun"],
              ].map(([zhLabel, enLabel], index) => {
                const label = text(zhLabel, enLabel);
                return (
                  <label key={zhLabel}>
                    <span>{label}</span>
                    <span class="quota-input">
                      <input
                        aria-label={text(
                          `${zhLabel}额度`,
                          `${enLabel} quota`,
                        )}
                        type="number"
                        min="0"
                        max="100"
                        step="0.5"
                        value={dailyLimits[index]}
                        onInput={(event) => {
                          const next = [...dailyLimits];
                          next[index] = event.currentTarget.valueAsNumber;
                          setDailyLimits(next);
                          setSaveError(false);
                        }}
                      />
                      <small>%</small>
                    </span>
                  </label>
                );
              })}
            </div>
            <div class={`quota-total${policyValid ? "" : " is-invalid"}`}>
              <span>{text("基础额度合计", "Base quota total")}</span>
              <strong>{Number.isFinite(total) ? total.toFixed(1) : "—"}%</strong>
            </div>
            <label class="settings-row">
              <span>
                <strong>
                  {text("工作日动态结转", "Dynamic workday carry")}
                </strong>
                <small>
                  {text(
                    "未知或超额日期不会产生新结转",
                    "Unknown or exceeded days never create new carry",
                  )}
                </small>
              </span>
              <input
                aria-label={text(
                  "工作日动态结转",
                  "Dynamic workday carry",
                )}
                type="checkbox"
                checked={carryEnabled}
                onChange={(event) => {
                  setCarryEnabled(event.currentTarget.checked);
                  setSaveError(false);
                }}
              />
            </label>
            <label class="timezone-field">
              <span>{text("自然日时区", "Policy timezone")}</span>
              <input
                type="text"
                aria-label={text("自然日时区", "Policy timezone")}
                value={policyTimezone}
                spellcheck={false}
                onInput={(event) => {
                  setPolicyTimezone(event.currentTarget.value);
                  setSaveError(false);
                }}
              />
            </label>
            {!policyValid ? (
              <p class="settings-error" role="alert">
                {text(
                  "请填写 7 天非负额度，基础额度合计不能超过 100%。",
                  "Enter seven non-negative daily quotas whose base total does not exceed 100%.",
                )}
              </p>
            ) : null}
          </section>
        ) : activeTab === "alerts" ? (
          <section
            id="settings-panel-alerts"
            role="tabpanel"
            aria-labelledby="settings-tab-alerts"
          >
            <div class="settings-section__heading alert-heading">
              <div>
                <span>{text("通知路由", "Notification routing")}</span>
                <h2 id="alert-settings">
                  {text("额度与重置提醒", "Quota and reset alerts")}
                </h2>
              </div>
              <div class="alert-channel-headings" aria-hidden="true">
                <span>{text("系统", "System")}</span>
                <span>{text("邮件", "Email")}</span>
              </div>
            </div>
            <div
              class={`notification-status notification-status--${settings.notificationPermissionStatus}`}
              role="status"
            >
              <span>{text("系统通知", "System notifications")}</span>
              {settings.notificationPermissionStatus !== "granted" &&
              onRequestNotificationPermission ? (
                <button
                  type="button"
                  onClick={() => {
                    void onRequestNotificationPermission().catch(() => undefined);
                  }}
                >
                  {settings.notificationPermissionStatus === "unknown"
                    ? text("启用系统通知", "Enable system notifications")
                    : text("重新检查权限", "Check permission again")}
                </button>
              ) : (
                <strong>
                  {
                    {
                      unknown: text(
                        "配置账号后可启用",
                        "Available after account setup",
                      ),
                      granted: text("已授权", "Authorized"),
                      denied: text(
                        "已拒绝 · 应用内提醒保留",
                        "Denied · In-app alerts retained",
                      ),
                      error: text(
                        "状态不可用 · 应用内提醒保留",
                        "Status unavailable · In-app alerts retained",
                      ),
                    }[settings.notificationPermissionStatus]
                  }
                </strong>
              )}
            </div>
            <div class="alert-matrix">
              {alertEvents.map((event) => (
                <div class="alert-row" key={event.kind}>
                  <span>
                    <strong>
                      {locale === "zh-CN" ? event.label : event.enLabel}
                    </strong>
                    <small>
                      {locale === "zh-CN" ? event.detail : event.enDetail}
                    </small>
                  </span>
                  {(["system", "email"] as const).map((channel) => {
                    const preference = alertPreferences.find(
                      (candidate) =>
                        candidate.eventKind === event.kind &&
                        candidate.channel === channel,
                    );
                    return (
                      <input
                        key={channel}
                        type="checkbox"
                        aria-label={text(
                          `${event.label} ${channel === "system" ? "系统" : "邮件"}提醒`,
                          `${event.enLabel} ${channel} alert`,
                        )}
                        checked={preference?.enabled ?? false}
                        onChange={(change) => {
                          setAlertPreference(
                            event.kind,
                            channel,
                            change.currentTarget.checked,
                          );
                        }}
                      />
                    );
                  })}
                </div>
              ))}
            </div>
            <p class="privacy-note">
              {text(
                "每个收件地址独立投递。密码只写入系统钥匙串，不进入数据库。",
                "Each recipient is delivered independently. The password is stored only in the system vault, never the database.",
              )}
            </p>
            <div class="smtp-settings">
              <div class="smtp-title">
                <span>
                  <strong>{text("发件邮箱", "Sender account")}</strong>
                  <small>
                    {settings.smtp.credentialStatus === "configured"
                      ? text("密码已安全保存", "Password stored securely")
                      : settings.smtp.credentialStatus === "unavailable"
                        ? text(
                            "系统钥匙串暂不可用",
                            "System vault is unavailable",
                          )
                        : text("尚未保存密码", "No password saved")}
                  </small>
                </span>
                <label class="compact-switch">
                  <span>{text("启用", "Enable")}</span>
                  <input
                    aria-label={text(
                      "启用邮件通知",
                      "Enable email notifications",
                    )}
                    type="checkbox"
                    checked={smtpEnabled}
                    onChange={(event) => {
                      setSmtpEnabled(event.currentTarget.checked);
                      setSaveError(false);
                    }}
                  />
                </label>
              </div>
              {smtpEnabled ? (
                <div class="smtp-fields">
                  <div class="smtp-grid">
                <label>
                  <span>{text("SMTP 主机", "SMTP host")}</span>
                  <input
                    aria-label={text("SMTP 主机", "SMTP host")}
                    type="text"
                    value={smtpHost}
                    placeholder="smtp.example.com"
                    spellcheck={false}
                    onInput={(event) => {
                      setSmtpHost(event.currentTarget.value);
                      setSaveError(false);
                    }}
                  />
                </label>
                <label>
                  <span>{text("端口", "Port")}</span>
                  <input
                    aria-label={text("SMTP 端口", "SMTP port")}
                    type="number"
                    min="1"
                    max="65535"
                    value={smtpPort}
                    onInput={(event) => {
                      setSmtpPort(event.currentTarget.value);
                      setSaveError(false);
                    }}
                  />
                </label>
                <label>
                  <span>{text("加密", "Encryption")}</span>
                  <select
                    aria-label={text("SMTP 加密", "SMTP encryption")}
                    value={smtpTlsMode}
                    onChange={(event) => {
                      setSmtpTlsMode(
                        event.currentTarget.value as "tls" | "starttls",
                      );
                      setSaveError(false);
                    }}
                  >
                    <option value="tls">TLS</option>
                    <option value="starttls">STARTTLS</option>
                  </select>
                </label>
              </div>
              <label>
                <span>{text("用户名", "Username")}</span>
                <input
                  aria-label={text("SMTP 用户名", "SMTP username")}
                  type="text"
                  value={smtpUsername}
                  autocomplete="off"
                  spellcheck={false}
                  onInput={(event) => {
                    setSmtpUsername(event.currentTarget.value);
                    setSaveError(false);
                  }}
                />
              </label>
              <div class="smtp-grid smtp-grid--sender">
                <label>
                  <span>{text("发件地址", "From address")}</span>
                  <input
                    aria-label={text(
                      "SMTP 发件地址",
                      "SMTP from address",
                    )}
                    type="email"
                    value={smtpFromAddress}
                    spellcheck={false}
                    onInput={(event) => {
                      setSmtpFromAddress(event.currentTarget.value);
                      setSaveError(false);
                    }}
                  />
                </label>
                <label>
                  <span>{text("显示名称", "Display name")}</span>
                  <input
                    aria-label={text("SMTP 发件名称", "SMTP sender name")}
                    type="text"
                    value={smtpFromName}
                    placeholder="QuotaTide"
                    onInput={(event) => {
                      setSmtpFromName(event.currentTarget.value);
                      setSaveError(false);
                    }}
                  />
                </label>
              </div>
              <label>
                <span>{text("应用密码", "App password")}</span>
                <input
                  aria-label={text("SMTP 应用密码", "SMTP app password")}
                  type="password"
                  value={smtpPassword}
                  placeholder={
                    settings.smtp.credentialStatus === "configured"
                      ? text(
                          "留空以保留现有密码",
                          "Leave blank to keep the saved password",
                        )
                      : text(
                          "输入应用专用密码",
                          "Enter an app-specific password",
                        )
                  }
                  autocomplete="new-password"
                  disabled={deleteSmtpPassword}
                  onInput={(event) => {
                    setSmtpPassword(event.currentTarget.value);
                    setDeleteSmtpPassword(false);
                    setSaveError(false);
                  }}
                />
              </label>
              {settings.smtp.credentialStatus !== "missing" ? (
                <label class="smtp-delete-secret">
                  <input
                    aria-label={text(
                      "删除已保存的 SMTP 密码",
                      "Delete saved SMTP password",
                    )}
                    type="checkbox"
                    checked={deleteSmtpPassword}
                    onChange={(event) => {
                      setDeleteSmtpPassword(event.currentTarget.checked);
                      if (event.currentTarget.checked) {
                        setSmtpPassword("");
                      }
                    }}
                  />
                  <span>
                    {text(
                      "保存时删除已保存的密码",
                      "Delete the saved password when saving",
                    )}
                  </span>
                </label>
              ) : null}
              <div class="smtp-recipients">
                <div class="smtp-recipients__head">
                  <span>{text("收件地址", "Recipients")}</span>
                  <button
                    type="button"
                    onClick={() => {
                      setSmtpRecipients((current) => [
                        ...current,
                        { address: "", enabled: true },
                      ]);
                    }}
                  >
                    {text("添加", "Add")}
                  </button>
                </div>
                {smtpRecipients.map((recipient, index) => (
                  <div class="smtp-recipient" key={index}>
                    <input
                      aria-label={text(
                        `收件地址 ${String(index + 1)}`,
                        `Recipient ${String(index + 1)}`,
                      )}
                      type="email"
                      value={recipient.address}
                      spellcheck={false}
                      onInput={(event) => {
                        setSmtpRecipients((current) =>
                          current.map((item, itemIndex) =>
                            itemIndex === index
                              ? {
                                  ...item,
                                  address: event.currentTarget.value,
                                }
                              : item,
                          ),
                        );
                        setSaveError(false);
                      }}
                    />
                    <input
                      aria-label={text(
                        `启用收件地址 ${String(index + 1)}`,
                        `Enable recipient ${String(index + 1)}`,
                      )}
                      type="checkbox"
                      checked={recipient.enabled}
                      onChange={(event) => {
                        setSmtpRecipients((current) =>
                          current.map((item, itemIndex) =>
                            itemIndex === index
                              ? {
                                  ...item,
                                  enabled: event.currentTarget.checked,
                                }
                              : item,
                          ),
                        );
                      }}
                    />
                    <button
                      type="button"
                      aria-label={text(
                        `删除收件地址 ${String(index + 1)}`,
                        `Delete recipient ${String(index + 1)}`,
                      )}
                      onClick={() => {
                        setSmtpRecipients((current) =>
                          current.filter((_, itemIndex) => itemIndex !== index),
                        );
                      }}
                    >
                      −
                    </button>
                  </div>
                ))}
              </div>
                  <div class="smtp-actions">
                    <button
                      type="button"
                      disabled={
                        !onSendTestEmail ||
                        testEmailState === "sending" ||
                        !settings.smtp.enabled ||
                        settings.smtp.credentialStatus !== "configured"
                      }
                      onClick={() => {
                        if (!onSendTestEmail) {
                          return;
                        }
                        setTestEmailState("sending");
                        void onSendTestEmail()
                          .then((count) => {
                            setTestEmailCount(count);
                            setTestEmailState("sent");
                          })
                          .catch(() => {
                            setTestEmailState("error");
                          });
                      }}
                    >
                      {testEmailState === "sending"
                        ? text("正在发送…", "Sending…")
                        : text("发送测试邮件", "Send test email")}
                    </button>
                    <span role="status">
                      {testEmailState === "sent"
                        ? text(
                            `已发送到 ${String(testEmailCount)} 个地址`,
                            `Sent to ${String(testEmailCount)} recipients`,
                          )
                        : testEmailState === "error"
                          ? text(
                              "发送失败，请检查 SMTP 设置",
                              "Delivery failed. Check SMTP settings.",
                            )
                          : text(
                              "先保存设置，再执行测试",
                              "Save settings before running a test",
                            )}
                    </span>
                  </div>
                </div>
              ) : (
                <p class="smtp-disabled-note">
                  {text(
                    "邮件通道已关闭。打开开关后再填写发件与收件信息。",
                    "Email delivery is off. Turn it on to configure sender and recipients.",
                  )}
                </p>
              )}
            </div>
            {!smtpValid ? (
              <p class="settings-error" role="alert">
                {text(
                  "启用邮件时，请填写有效主机、端口、账号、发件地址和至少一个收件地址。",
                  "When email is enabled, enter a valid host, port, account, sender address, and at least one recipient.",
                )}
              </p>
            ) : null}
          </section>
        ) : (
          <section
            id="settings-panel-privacy"
            role="tabpanel"
            aria-labelledby="settings-tab-privacy"
          >
            <PrivacyPanel
              onExportDiagnostics={onExportDiagnostics}
              onClearLocalData={onClearLocalData}
            />
          </section>
        )}
        {saveError ? (
          <p class="settings-error settings-error--floating" role="alert">
            {text(
              "设置未保存。若其他窗口已修改设置，当前值已重新载入，请确认后再试。",
              "Settings were not saved. If another window changed them, the latest revision has been reloaded; review and try again.",
            )}
          </p>
        ) : null}
      </main>

      <footer class="ledger-footer settings-footer">
        <button type="button" onClick={requestBack}>
          {discardRequested
            ? text("确认放弃", "Discard")
            : hasChanges
              ? text("放弃更改", "Discard changes")
              : text("取消", "Cancel")}
        </button>
        <span
          class={`settings-save-state${hasChanges ? " is-dirty" : ""}${discardRequested ? " is-warning" : ""}`}
          role="status"
        >
          <i aria-hidden="true" />
          {discardRequested
            ? text("再次点击以放弃", "Click again to discard")
            : hasChanges
              ? text("有未保存更改", "Unsaved changes")
              : text("所有更改已保存", "All changes saved")}
        </span>
        <button
          type="button"
          class="settings-save"
          disabled={saving || !hasChanges}
          onClick={save}
        >
          {saving
            ? text("正在保存…", "Saving…")
            : text("保存全部设置", "Save all settings")}
        </button>
      </footer>
    </article>
  );
}

export function TrayApp({
  fixture,
  settings = unconfiguredSettings,
  alerts = null,
  focusRequest = null,
  externalRefreshing = false,
  recoveredFromBackup = false,
  updateState,
  onHide,
  onRefresh,
  onDismissAlert,
  onDismissAllAlerts,
  onWeekDetailChange,
  onPickAuthFile,
  onRequestNotificationPermission,
  onSendTestEmail,
  onExportDiagnostics,
  onClearLocalData,
  onSaveSettings,
  onReloadSettings,
  onCheckForUpdate,
  onInstallUpdate,
}: TrayAppProps) {
  const { text } = useI18n();
  const [view, setView] = useState<"ledger" | "settings">("ledger");
  const [theme, setTheme] = useState<ColorTheme>(readInitialColorTheme);
  const [currentSettings, setCurrentSettings] = useState(settings);
  const [refreshing, setRefreshing] = useState(false);
  const [coolingDown, setCoolingDown] = useState(false);
  const refreshingRef = useRef(false);
  const coolingDownRef = useRef(false);
  const cooldownTimerRef = useRef<number | undefined>(undefined);

  useEffect(() => {
    applyColorTheme(theme);
  }, [theme]);

  const toggleTheme = useCallback(() => {
    setTheme((current) => {
      const next = current === "dark" ? "light" : "dark";
      applyColorTheme(next);
      rememberColorTheme(next);
      return next;
    });
  }, []);

  useEffect(
    () => () => {
      if (cooldownTimerRef.current !== undefined) {
        window.clearTimeout(cooldownTimerRef.current);
      }
    },
    [],
  );

  useEffect(() => {
    setCurrentSettings(settings);
  }, [settings]);

  useEffect(() => {
    void onWeekDetailChange?.(view === "settings");
  }, [onWeekDetailChange, view]);

  useEffect(() => {
    if (focusRequest !== null) {
      setView("ledger");
    }
  }, [focusRequest?.activationId]);

  const handleRefresh = useCallback(() => {
    if (refreshingRef.current || externalRefreshing || coolingDownRef.current) {
      return;
    }

    refreshingRef.current = true;
    setRefreshing(true);
    let refreshResult: unknown;
    try {
      refreshResult = onRefresh();
    } catch {
      refreshResult = undefined;
    }
    void Promise.resolve(refreshResult)
      .catch(() => undefined)
      .then((cooldownMs) => {
        refreshingRef.current = false;
        setRefreshing(false);
        if (typeof cooldownMs === "number" && cooldownMs > 0) {
          coolingDownRef.current = true;
          setCoolingDown(true);
          cooldownTimerRef.current = window.setTimeout(() => {
            coolingDownRef.current = false;
            setCoolingDown(false);
            cooldownTimerRef.current = undefined;
          }, cooldownMs);
        }
      });
  }, [externalRefreshing, onRefresh]);

  const visibleRefreshing = refreshing || externalRefreshing;

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      const command = event.metaKey || event.ctrlKey;

      if (command && event.key === ",") {
        event.preventDefault();
        setView("settings");
        return;
      }

      if (command && event.key.toLowerCase() === "r") {
        event.preventDefault();
        handleRefresh();
        return;
      }

      if (event.key === "Escape") {
        event.preventDefault();
        if (view === "settings") {
          setView("ledger");
        } else {
          onHide();
        }
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [handleRefresh, onHide, view]);

  if (view === "settings") {
    return (
      <SettingsView
        settings={currentSettings}
        theme={theme}
        onToggleTheme={toggleTheme}
        onBack={() => {
          setView("ledger");
        }}
        onPickAuthFile={onPickAuthFile}
        onRequestNotificationPermission={
          onRequestNotificationPermission
            ? async () => {
                const status = await onRequestNotificationPermission();
                setCurrentSettings((current) => ({
                  ...current,
                  notificationPermissionStatus: status,
                }));
              }
            : undefined
        }
        onSendTestEmail={onSendTestEmail}
        onExportDiagnostics={onExportDiagnostics}
        onClearLocalData={onClearLocalData}
        updateState={updateState}
        onCheckForUpdate={onCheckForUpdate}
        onInstallUpdate={onInstallUpdate}
        onSave={async (draft) => {
          if (!onSaveSettings) {
            return;
          }
          try {
            setCurrentSettings(await onSaveSettings(draft));
          } catch (error) {
            if (isSettingsConflict(error) && onReloadSettings) {
              setCurrentSettings(await onReloadSettings());
            }
            throw error;
          }
        }}
      />
    );
  }

  return (
    <>
      {recoveredFromBackup ? (
        <div class="recovery-success-banner" role="status">
          {text(
            "已从最近的有效备份恢复本地账本；损坏副本仍保留在数据目录中。",
            "The local ledger was restored from the newest valid backup; the damaged copy remains in the data directory.",
          )}
        </div>
      ) : null}
      <WeeklyLedger
        fixture={fixture}
        storyTheme={currentSettings.storyTheme}
        theme={theme}
        onToggleTheme={toggleTheme}
        alerts={alerts}
        focusTarget={focusRequest?.target}
        focusActivationId={focusRequest?.activationId}
        onDismissAlert={onDismissAlert}
        onDismissAllAlerts={onDismissAllAlerts}
        onWeekDetailChange={onWeekDetailChange}
        onOpenSettings={() => {
          setView("settings");
        }}
        onRefresh={handleRefresh}
        refreshing={visibleRefreshing}
        refreshDisabled={visibleRefreshing || coolingDown}
      />
    </>
  );
}

function isSettingsConflict(error: unknown): boolean {
  return (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    error.code === "settings_conflict"
  );
}
