import { useCallback, useEffect, useRef, useState } from "preact/hooks";

import type { PublicAccountSettings } from "./bindings/PublicAccountSettings";
import { WeeklyLedger, type LedgerFixture } from "./WeeklyLedger";

const unconfiguredAccount: PublicAccountSettings = {
  settingsRevision: 0,
  configured: false,
  pathSummary: null,
  accountLabel: null,
};

type TrayAppProps = {
  fixture: LedgerFixture;
  accountSettings?: PublicAccountSettings;
  onHide: () => void;
  onRefresh: () => unknown;
  onSelectAuth?: (
    expectedSettingsRevision: number,
  ) => Promise<PublicAccountSettings>;
  onReloadAccount?: () => Promise<PublicAccountSettings>;
};

function SettingsView({
  accountSettings,
  onBack,
  onSelectAuth,
}: {
  accountSettings: PublicAccountSettings;
  onBack: () => void;
  onSelectAuth: () => Promise<void>;
}) {
  const [activeTab, setActiveTab] = useState<
    "quota" | "account" | "notifications"
  >("account");
  const [selectingAuth, setSelectingAuth] = useState(false);
  const [authError, setAuthError] = useState(false);

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
              工作日未用完的额度会平分到本窗口后续工作日。
            </p>
            <strong class="settings-value">16 · 16 · 16 · 16 · 16 · 10 · 10</strong>
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
  onHide,
  onRefresh,
  onSelectAuth,
  onReloadAccount,
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
    if (refreshingRef.current || coolingDownRef.current) {
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
  }, [onRefresh]);

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
      refreshing={refreshing}
      refreshDisabled={refreshing || coolingDown}
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
