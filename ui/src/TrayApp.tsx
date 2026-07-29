import { useCallback, useEffect, useRef, useState } from "preact/hooks";

import type { PublicAccountSettings } from "./bindings/PublicAccountSettings";
import type { QuotaPolicyDraft } from "./bindings/QuotaPolicyDraft";
import { WeeklyLedger, type LedgerFixture } from "./WeeklyLedger";

const unconfiguredAccount: PublicAccountSettings = {
  settingsRevision: 0,
  configured: false,
  pathSummary: null,
  accountLabel: null,
  quotaPolicy: {
    policyRevision: 1,
    policyTimezone: "Asia/Shanghai",
    carryWorkdaysEnabled: true,
    baseMicropoints: [
      16_000_000, 16_000_000, 16_000_000, 16_000_000, 16_000_000,
      10_000_000, 10_000_000,
    ],
  },
};

type TrayAppProps = {
  fixture: LedgerFixture;
  accountSettings?: PublicAccountSettings;
  externalRefreshing?: boolean;
  onHide: () => void;
  onRefresh: () => unknown;
  onSelectAuth?: (
    expectedSettingsRevision: number,
  ) => Promise<PublicAccountSettings>;
  onReloadAccount?: () => Promise<PublicAccountSettings>;
  onUpdatePolicy?: (
    expectedSettingsRevision: number,
    draft: QuotaPolicyDraft,
  ) => Promise<PublicAccountSettings>;
};

function SettingsView({
  accountSettings,
  onBack,
  onSelectAuth,
  onUpdatePolicy,
}: {
  accountSettings: PublicAccountSettings;
  onBack: () => void;
  onSelectAuth: () => Promise<void>;
  onUpdatePolicy: (draft: QuotaPolicyDraft) => Promise<void>;
}) {
  const [activeTab, setActiveTab] = useState<
    "quota" | "account" | "notifications"
  >("account");
  const [selectingAuth, setSelectingAuth] = useState(false);
  const [authError, setAuthError] = useState(false);
  const [dailyLimits, setDailyLimits] = useState(() =>
    accountSettings.quotaPolicy.baseMicropoints.map((value) => value / 1_000_000),
  );
  const [policyTimezone, setPolicyTimezone] = useState(
    accountSettings.quotaPolicy.policyTimezone,
  );
  const [carryEnabled, setCarryEnabled] = useState(
    accountSettings.quotaPolicy.carryWorkdaysEnabled,
  );
  const [savingPolicy, setSavingPolicy] = useState(false);
  const [policyError, setPolicyError] = useState(false);
  const total = dailyLimits.reduce((sum, value) => sum + value, 0);
  const policyValid =
    dailyLimits.length === 7 &&
    dailyLimits.every((value) => Number.isFinite(value) && value >= 0) &&
    total <= 100 &&
    policyTimezone.trim().length > 0;

  const handleSelectAuth = () => {
    if (selectingAuth) {
      return;
    }
    setSelectingAuth(true);
    setAuthError(false);
    void onSelectAuth()
      .catch(() => {
        setAuthError(true);
      })
      .finally(() => {
        setSelectingAuth(false);
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
          <p>QuotaTide 偏好设置</p>
        </div>
      </header>

      <main class="settings-content">
        <div class="settings-tabs" role="tablist" aria-label="设置分类">
          {(
            [
              ["quota", "额度"],
              ["account", "账号"],
              ["notifications", "通知"],
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
              <span>账号</span>
              <h2 id="account-settings">Codex 数据源</h2>
            </div>
            <div class="account-status" aria-live="polite">
              <span>
                {accountSettings.configured
                  ? accountSettings.accountLabel
                  : "尚未配置 Codex 账号"}
              </span>
              {accountSettings.pathSummary ? (
                <strong>{accountSettings.pathSummary}</strong>
              ) : null}
            </div>
            <button
              type="button"
              class="primary-action"
              disabled={selectingAuth}
              onClick={handleSelectAuth}
            >
              {selectingAuth
                ? "正在验证…"
                : accountSettings.configured
                  ? "更换 auth.json"
                  : "选择 auth.json"}
            </button>
            {authError ? (
              <p class="settings-error" role="alert">
                无法验证该文件。请选择 Codex 自动维护的 auth.json。
              </p>
            ) : null}
            <p class="privacy-note">
              只读访问。QuotaTide 不会修改 auth.json，也不会上传令牌。
            </p>
          </section>
        ) : activeTab === "quota" ? (
          <section aria-labelledby="quota-settings">
            <div class="settings-section__heading">
              <span>额度</span>
              <h2 id="quota-settings">当前七日策略模板</h2>
            </div>
            <p class="settings-description">
              每日基础额度合计不超过 100%。已确认的工作日余量只会平分给同一自然周后续工作日。
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
                          setPolicyError(false);
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
                  setPolicyError(false);
                }}
              />
            </label>
            <button
              type="button"
              class="primary-action"
              disabled={!policyValid || savingPolicy}
              onClick={() => {
                if (!policyValid || savingPolicy) {
                  return;
                }
                setSavingPolicy(true);
                setPolicyError(false);
                void onUpdatePolicy({
                  policyTimezone: policyTimezone.trim(),
                  carryWorkdaysEnabled: carryEnabled,
                  baseMicropoints: dailyLimits.map((value) =>
                    Math.round(value * 1_000_000),
                  ),
                })
                  .catch(() => {
                    setPolicyError(true);
                  })
                  .finally(() => {
                    setSavingPolicy(false);
                  });
              }}
            >
              {savingPolicy ? "正在保存…" : "保存额度策略"}
            </button>
            {!policyValid ? (
              <p class="settings-error" role="alert">
                请填写 7 天非负额度，基础额度合计不能超过 100%。
              </p>
            ) : policyError ? (
              <p class="settings-error" role="alert">
                策略未保存，请检查时区或重新载入后再试。
              </p>
            ) : null}
          </section>
        ) : (
          <section aria-labelledby="notification-settings">
            <div class="settings-section__heading">
              <span>通知</span>
              <h2 id="notification-settings">额度与重置提醒</h2>
            </div>
            <label class="settings-row">
              <span>
                <strong>系统通知</strong>
                <small>接近额度或预计重置时提醒</small>
              </span>
              <input type="checkbox" defaultChecked />
            </label>
          </section>
        )}
      </main>

      <footer class="ledger-footer settings-footer">
        <button type="button" onClick={onBack}>
          完成
        </button>
        <span>⌘/Ctrl + , 打开设置</span>
      </footer>
    </article>
  );
}

export function TrayApp({
  fixture,
  accountSettings = unconfiguredAccount,
  externalRefreshing = false,
  onHide,
  onRefresh,
  onSelectAuth,
  onReloadAccount,
  onUpdatePolicy,
}: TrayAppProps) {
  const [view, setView] = useState<"ledger" | "settings">("ledger");
  const [currentAccount, setCurrentAccount] = useState(accountSettings);
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
    setCurrentAccount(accountSettings);
  }, [accountSettings]);

  const handleRefresh = useCallback(() => {
    if (
      refreshingRef.current ||
      externalRefreshing ||
      coolingDownRef.current
    ) {
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
        accountSettings={currentAccount}
        onBack={() => {
          setView("ledger");
        }}
        onSelectAuth={async () => {
          if (onSelectAuth) {
            try {
              setCurrentAccount(
                await onSelectAuth(currentAccount.settingsRevision),
              );
            } catch (error) {
              if (isSettingsConflict(error) && onReloadAccount) {
                setCurrentAccount(await onReloadAccount());
              }
              throw error;
            }
          }
        }}
        onUpdatePolicy={async (draft) => {
          if (!onUpdatePolicy) {
            return;
          }
          try {
            setCurrentAccount(
              await onUpdatePolicy(currentAccount.settingsRevision, draft),
            );
          } catch (error) {
            if (isSettingsConflict(error) && onReloadAccount) {
              setCurrentAccount(await onReloadAccount());
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
