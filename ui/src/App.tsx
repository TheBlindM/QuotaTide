import { useEffect, useState } from "preact/hooks";

import type { BuildInfo } from "./bindings/BuildInfo";
import { loadBuildInfo } from "./api/build-info";
import { hideMainWindow, requestManualRefresh } from "./api/tray-shell";
import { TrayApp } from "./TrayApp";
import {
  ledgerFixtures,
  type LedgerTone,
} from "./WeeklyLedger";

type ViewState =
  | { kind: "loading" }
  | { kind: "ready"; info: BuildInfo }
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
        }
      : { kind: "loading" },
  );

  useEffect(() => {
    if (isPreview) {
      return;
    }

    let active = true;

    void loadBuildInfo()
      .then((info) => {
        if (active) {
          setState({ kind: "ready", info });
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

    document.documentElement.dataset.surface =
      surface === "opaque" ? "opaque" : "glass";
  }, []);

  if (state.kind === "loading") {
    return <main class="boot-state" aria-busy="true">正在连接 Rust 核心…</main>;
  }

  if (state.kind === "error") {
    return (
      <main>
        <p role="alert">桌面外壳不可用，请从任务栏菜单重试。</p>
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
      onHide={() => {
        void hideMainWindow().catch(() => undefined);
      }}
      onRefresh={() => {
        void requestManualRefresh().catch(() => undefined);
      }}
    />
  );
}
