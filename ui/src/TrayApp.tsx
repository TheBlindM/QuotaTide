import { useCallback, useEffect, useRef, useState } from "preact/hooks";

import { WeeklyLedger, type LedgerFixture } from "./WeeklyLedger";

type TrayAppProps = {
  fixture: LedgerFixture;
  onHide: () => void;
  onRefresh: () => void | Promise<void>;
};

function SettingsView({ onBack }: { onBack: () => void }) {
  const [activeTab, setActiveTab] = useState<
    "quota" | "account" | "notifications"
  >("account");

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
            <label>
              <span>auth.json 路径</span>
              <input
                aria-label="auth.json 路径"
                type="text"
                defaultValue="~/.codex/auth.json"
                spellcheck={false}
              />
            </label>
            <p class="privacy-note">
              只读访问。QuotaTide 不会修改 auth.json，也不会上传令牌。
            </p>
          </section>
        ) : activeTab === "quota" ? (
          <section aria-labelledby="quota-settings">
            <div class="settings-section__heading">
              <span>额度</span>
              <h2 id="quota-settings">当前七日策略</h2>
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

export function TrayApp({ fixture, onHide, onRefresh }: TrayAppProps) {
  const [view, setView] = useState<"ledger" | "settings">("ledger");
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

  const handleRefresh = useCallback(() => {
    if (refreshingRef.current || coolingDownRef.current) {
      return;
    }

    refreshingRef.current = true;
    setRefreshing(true);
    let refreshResult: void | Promise<void>;
    try {
      refreshResult = onRefresh();
    } catch {
      refreshResult = undefined;
    }
    void Promise.resolve(refreshResult)
      .catch(() => undefined)
      .finally(() => {
        refreshingRef.current = false;
        coolingDownRef.current = true;
        setRefreshing(false);
        setCoolingDown(true);
        cooldownTimerRef.current = window.setTimeout(() => {
          coolingDownRef.current = false;
          setCoolingDown(false);
          cooldownTimerRef.current = undefined;
        }, 30_000);
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
        onBack={() => {
          setView("ledger");
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
