import { useCallback, useEffect, useRef, useState } from "preact/hooks";

import type { AlertChannel } from "./bindings/AlertChannel";
import type { AlertEventKind } from "./bindings/AlertEventKind";
import type { AlertPreferenceDraft } from "./bindings/AlertPreferenceDraft";
import type { NotificationPermissionStatus } from "./bindings/NotificationPermissionStatus";
import type { PublicAlertInbox } from "./bindings/PublicAlertInbox";
import type { PublicSettings } from "./bindings/PublicSettings";
import type { SettingsDraft } from "./bindings/SettingsDraft";
import type { NotificationActivation } from "./api/alerts";
import { WeeklyLedger, type LedgerFixture } from "./WeeklyLedger";

const alertEvents: ReadonlyArray<{
  kind: AlertEventKind;
  label: string;
  detail: string;
}> = [
  {
    kind: "daily_80",
    label: "每日额度达到 80%",
    detail: "今日实际使用接近动态上限",
  },
  {
    kind: "daily_100",
    label: "每日额度达到 100%",
    detail: "今日实际使用达到动态上限",
  },
  {
    kind: "weekly_remaining_20",
    label: "周额度剩余 20%",
    detail: "当前七日窗口进入注意区间",
  },
  {
    kind: "weekly_remaining_10",
    label: "周额度剩余 10%",
    detail: "当前七日窗口接近耗尽",
  },
  {
    kind: "radar_chance_70",
    label: "重置预测达到 70%",
    detail: "Reset Radar 预测近期可能重置",
  },
  {
    kind: "quota_reset_confirmed",
    label: "额度重置已确认",
    detail: "本机连续观测确认新额度窗口",
  },
  {
    kind: "source_failures_3",
    label: "连续 3 次采集失败",
    detail: "Codex 或 Reset Radar 暂时不可用",
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

type TrayAppProps = {
  fixture: LedgerFixture;
  settings?: PublicSettings;
  alerts?: PublicAlertInbox | null;
  focusRequest?: NotificationActivation | null;
  externalRefreshing?: boolean;
  onHide: () => void;
  onRefresh: () => unknown;
  onRequestNotificationPermission?: () => Promise<NotificationPermissionStatus>;
  onSendTestEmail?: () => Promise<number>;
  onSaveSettings?: (draft: SettingsDraft) => Promise<PublicSettings>;
  onReloadSettings?: () => Promise<PublicSettings>;
};

function SettingsView({
  settings,
  onBack,
  onRequestNotificationPermission,
  onSendTestEmail,
  onSave,
}: {
  settings: PublicSettings;
  onBack: () => void;
  onRequestNotificationPermission?: () => Promise<void>;
  onSendTestEmail?: () => Promise<number>;
  onSave: (draft: SettingsDraft) => Promise<void>;
}) {
  const [activeTab, setActiveTab] = useState<"account" | "quota" | "alerts">(
    "account",
  );
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
  }, [settings.settingsRevision]);

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
  const settingsValid = policyValid && smtpValid;

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
    if (!settingsValid || saving) {
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
        baseMicropoints: dailyLimits.map((value) =>
          Math.round(value * 1_000_000),
        ),
      },
      alertPreferences,
      autostartEnabled,
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

  return (
    <article class="settings-view">
      <header class="settings-header">
        <button type="button" aria-label="返回" onClick={onBack}>
          ←
        </button>
        <div>
          <h1>设置</h1>
          <p>所有更改将作为一个版本保存</p>
        </div>
      </header>

      <main class="settings-content">
        <div class="settings-tabs" role="tablist" aria-label="设置分类">
          {(
            [
              ["account", "账号"],
              ["quota", "额度"],
              ["alerts", "提醒"],
            ] as const
          ).map(([tab, label]) => (
            <button
              key={tab}
              type="button"
              role="tab"
              aria-selected={activeTab === tab}
              onClick={() => {
                setActiveTab(tab);
              }}
            >
              {label}
            </button>
          ))}
        </div>

        {activeTab === "account" ? (
          <section aria-labelledby="account-settings">
            <div class="settings-section__heading">
              <span>数据源</span>
              <h2 id="account-settings">当前 Codex 账号</h2>
            </div>
            <div class="account-status" aria-live="polite">
              <span>
                {settings.configured
                  ? settings.accountLabel
                  : "尚未配置 Codex 账号"}
              </span>
              {settings.pathSummary ? <strong>{settings.pathSummary}</strong> : null}
            </div>
            <label class="auth-path-field">
              <span>auth.json 路径</span>
              <input
                type="text"
                aria-label="auth.json 路径"
                value={authPath}
                placeholder={
                  settings.configured
                    ? "留空以保留当前文件"
                    : "/Users/name/.codex/auth.json"
                }
                autocomplete="off"
                spellcheck={false}
                onInput={(event) => {
                  setAuthPath(event.currentTarget.value);
                  setSaveError(false);
                }}
              />
            </label>
            <p class="privacy-note">
              只读访问。QuotaTide 不会修改 auth.json，也不会把路径或令牌发给网页。
            </p>
            <label class="settings-row settings-row--separated">
              <span>
                <strong>登录后自动启动</strong>
                <small>仅在当前 macOS 或 Windows 用户下运行</small>
              </span>
              <input
                aria-label="登录后自动启动"
                type="checkbox"
                checked={autostartEnabled}
                onChange={(event) => {
                  setAutostartEnabled(event.currentTarget.checked);
                  setSaveError(false);
                }}
              />
            </label>
          </section>
        ) : activeTab === "quota" ? (
          <section aria-labelledby="quota-settings">
            <div class="settings-section__heading">
              <span>七日模板</span>
              <h2 id="quota-settings">每日基础额度</h2>
            </div>
            <p class="settings-description">
              七天合计不超过 100%。未用完的工作日额度可平分给同一周后续工作日。
            </p>
            <div class="quota-day-grid">
              {["周一", "周二", "周三", "周四", "周五", "周六", "周日"].map(
                (label, index) => (
                  <label key={label}>
                    <span>{label}</span>
                    <span class="quota-input">
                      <input
                        aria-label={`${label}额度`}
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
                ),
              )}
            </div>
            <div class={`quota-total${policyValid ? "" : " is-invalid"}`}>
              <span>基础额度合计</span>
              <strong>{Number.isFinite(total) ? total.toFixed(1) : "—"}%</strong>
            </div>
            <label class="settings-row">
              <span>
                <strong>工作日动态结转</strong>
                <small>未知或超额日期不会产生新结转</small>
              </span>
              <input
                aria-label="工作日动态结转"
                type="checkbox"
                checked={carryEnabled}
                onChange={(event) => {
                  setCarryEnabled(event.currentTarget.checked);
                  setSaveError(false);
                }}
              />
            </label>
            <label class="timezone-field">
              <span>自然日时区</span>
              <input
                type="text"
                aria-label="自然日时区"
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
                请填写 7 天非负额度，基础额度合计不能超过 100%。
              </p>
            ) : null}
          </section>
        ) : (
          <section aria-labelledby="alert-settings">
            <div class="settings-section__heading alert-heading">
              <div>
                <span>通知路由</span>
                <h2 id="alert-settings">额度与重置提醒</h2>
              </div>
              <div class="alert-channel-headings" aria-hidden="true">
                <span>系统</span>
                <span>邮件</span>
              </div>
            </div>
            <div
              class={`notification-status notification-status--${settings.notificationPermissionStatus}`}
              role="status"
            >
              <span>系统通知</span>
              {settings.notificationPermissionStatus !== "granted" &&
              onRequestNotificationPermission ? (
                <button
                  type="button"
                  onClick={() => {
                    void onRequestNotificationPermission().catch(() => undefined);
                  }}
                >
                  {settings.notificationPermissionStatus === "unknown"
                    ? "启用系统通知"
                    : "重新检查权限"}
                </button>
              ) : (
                <strong>
                  {{
                    unknown: "配置账号后可启用",
                    granted: "已授权",
                    denied: "已拒绝 · 应用内提醒保留",
                    error: "状态不可用 · 应用内提醒保留",
                  }[settings.notificationPermissionStatus]}
                </strong>
              )}
            </div>
            <div class="alert-matrix">
              {alertEvents.map((event) => (
                <div class="alert-row" key={event.kind}>
                  <span>
                    <strong>{event.label}</strong>
                    <small>{event.detail}</small>
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
                        aria-label={`${event.label} ${
                          channel === "system" ? "系统" : "邮件"
                        }提醒`}
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
              每个收件地址独立投递。密码只写入系统钥匙串，不进入数据库。
            </p>
            <div class="smtp-settings">
              <div class="smtp-title">
                <span>
                  <strong>发件邮箱</strong>
                  <small>
                    {settings.smtp.credentialStatus === "configured"
                      ? "密码已安全保存"
                      : settings.smtp.credentialStatus === "unavailable"
                        ? "系统钥匙串暂不可用"
                        : "尚未保存密码"}
                  </small>
                </span>
                <label class="compact-switch">
                  <span>启用</span>
                  <input
                    aria-label="启用邮件通知"
                    type="checkbox"
                    checked={smtpEnabled}
                    onChange={(event) => {
                      setSmtpEnabled(event.currentTarget.checked);
                      setSaveError(false);
                    }}
                  />
                </label>
              </div>
              <div class="smtp-grid">
                <label>
                  <span>SMTP 主机</span>
                  <input
                    aria-label="SMTP 主机"
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
                  <span>端口</span>
                  <input
                    aria-label="SMTP 端口"
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
                  <span>加密</span>
                  <select
                    aria-label="SMTP 加密"
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
                <span>用户名</span>
                <input
                  aria-label="SMTP 用户名"
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
                  <span>发件地址</span>
                  <input
                    aria-label="SMTP 发件地址"
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
                  <span>显示名称</span>
                  <input
                    aria-label="SMTP 发件名称"
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
                <span>应用密码</span>
                <input
                  aria-label="SMTP 应用密码"
                  type="password"
                  value={smtpPassword}
                  placeholder={
                    settings.smtp.credentialStatus === "configured"
                      ? "留空以保留现有密码"
                      : "输入应用专用密码"
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
                    aria-label="删除已保存的 SMTP 密码"
                    type="checkbox"
                    checked={deleteSmtpPassword}
                    onChange={(event) => {
                      setDeleteSmtpPassword(event.currentTarget.checked);
                      if (event.currentTarget.checked) {
                        setSmtpPassword("");
                      }
                    }}
                  />
                  <span>保存时删除已保存的密码</span>
                </label>
              ) : null}
              <div class="smtp-recipients">
                <div class="smtp-recipients__head">
                  <span>收件地址</span>
                  <button
                    type="button"
                    onClick={() => {
                      setSmtpRecipients((current) => [
                        ...current,
                        { address: "", enabled: true },
                      ]);
                    }}
                  >
                    添加
                  </button>
                </div>
                {smtpRecipients.map((recipient, index) => (
                  <div class="smtp-recipient" key={index}>
                    <input
                      aria-label={`收件地址 ${String(index + 1)}`}
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
                      aria-label={`启用收件地址 ${String(index + 1)}`}
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
                      aria-label={`删除收件地址 ${String(index + 1)}`}
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
                  {testEmailState === "sending" ? "正在发送…" : "发送测试邮件"}
                </button>
                <span role="status">
                  {testEmailState === "sent"
                    ? `已发送到 ${String(testEmailCount)} 个地址`
                    : testEmailState === "error"
                      ? "发送失败，请检查 SMTP 设置"
                      : "先保存设置，再执行测试"}
                </span>
              </div>
            </div>
            {!smtpValid ? (
              <p class="settings-error" role="alert">
                启用邮件时，请填写有效主机、端口、账号、发件地址和至少一个收件地址。
              </p>
            ) : null}
          </section>
        )}
        {saveError ? (
          <p class="settings-error settings-error--floating" role="alert">
            设置未保存。若其他窗口已修改设置，当前值已重新载入，请确认后再试。
          </p>
        ) : null}
      </main>

      <footer class="ledger-footer settings-footer">
        <button type="button" onClick={onBack}>
          取消
        </button>
        <span>账号、策略与提醒一次提交</span>
        <button
          type="button"
          class="settings-save"
          disabled={!settingsValid || saving}
          onClick={save}
        >
          {saving ? "正在保存…" : "保存全部设置"}
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
  onHide,
  onRefresh,
  onRequestNotificationPermission,
  onSendTestEmail,
  onSaveSettings,
  onReloadSettings,
}: TrayAppProps) {
  const [view, setView] = useState<"ledger" | "settings">("ledger");
  const [currentSettings, setCurrentSettings] = useState(settings);
  const [refreshing, setRefreshing] = useState(false);
  const [coolingDown, setCoolingDown] = useState(false);
  const refreshingRef = useRef(false);
  const coolingDownRef = useRef(false);
  const cooldownTimerRef = useRef<number | undefined>(undefined);

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
        onBack={() => {
          setView("ledger");
        }}
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
    <WeeklyLedger
      fixture={fixture}
      alerts={alerts}
      focusTarget={focusRequest?.target}
      focusActivationId={focusRequest?.activationId}
      onOpenSettings={() => {
        setView("settings");
      }}
      onRefresh={handleRefresh}
      refreshing={visibleRefreshing}
      refreshDisabled={visibleRefreshing || coolingDown}
    />
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
