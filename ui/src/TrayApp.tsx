import { useEffect, useState } from "preact/hooks";

import { WeeklyLedger, type LedgerFixture } from "./WeeklyLedger";

type TrayAppProps = {
  fixture: LedgerFixture;
  onHide: () => void;
  onRefresh: () => void;
};

function SettingsView({ onBack }: { onBack: () => void }) {
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

        <section aria-labelledby="display-settings">
          <div class="settings-section__heading">
            <span>显示</span>
            <h2 id="display-settings">外观与辅助功能</h2>
          </div>
          <label class="settings-row">
            <span>
              <strong>跟随系统外观</strong>
              <small>自动使用浅色或深色主题</small>
            </span>
            <input type="checkbox" defaultChecked />
          </label>
          <label class="settings-row">
            <span>
              <strong>降低动态效果</strong>
              <small>同时尊重系统的减少动态效果设置</small>
            </span>
            <input type="checkbox" />
          </label>
        </section>
      </main>

      <footer class="ledger-footer settings-footer">
        <button type="button" onClick={onBack}>
          返回额度
        </button>
        <span>⌘/Ctrl + , 打开设置</span>
      </footer>
    </article>
  );
}

export function TrayApp({ fixture, onHide, onRefresh }: TrayAppProps) {
  const [view, setView] = useState<"ledger" | "settings">("ledger");

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
        onRefresh();
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
  }, [onHide, onRefresh, view]);

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
      onRefresh={onRefresh}
    />
  );
}
