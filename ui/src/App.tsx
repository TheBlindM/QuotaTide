import { useEffect, useState } from "preact/hooks";

import type { BuildInfo } from "./bindings/BuildInfo";
import type { PublicAccountSettings } from "./bindings/PublicAccountSettings";
import {
  getAccountSettings,
  selectAuthFile,
} from "./api/account-settings";
import { loadBuildInfo } from "./api/build-info";
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
          },
        }
      : { kind: "loading" },
  );

  useEffect(() => {
    if (isPreview) {
      return;
    }

    let active = true;

    void Promise.all([loadBuildInfo(), getAccountSettings()])
      .then(([info, accountSettings]) => {
        if (active) {
          setState({ kind: "ready", info, accountSettings });
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

  return (
    <TrayApp
      fixture={ledgerFixtures[tone]}
      accountSettings={state.accountSettings}
      onHide={() => {
        void hideMainWindow().catch(() => undefined);
      }}
      onRefresh={() => {
        return requestManualRefresh().catch(() => undefined);
      }}
      onSelectAuth={selectAuthFile}
    />
  );
}
