import { useEffect, useState } from "preact/hooks";

import type { BuildInfo } from "./bindings/BuildInfo";
import { loadBuildInfo } from "./api/build-info";

type ViewState =
  | { kind: "loading" }
  | { kind: "ready"; info: BuildInfo }
  | { kind: "error" };

export function App() {
  const [state, setState] = useState<ViewState>({ kind: "loading" });

  useEffect(() => {
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
  }, []);

  if (state.kind === "loading") {
    return <main aria-busy="true">Connecting to the Rust core…</main>;
  }

  if (state.kind === "error") {
    return (
      <main>
        <p role="alert">The desktop shell is unavailable.</p>
      </main>
    );
  }

  return (
    <main>
      <section class="skeleton-card" aria-labelledby="product-name">
        <img
          class="skeleton-card__mark"
          src="/quotatide-mark.svg"
          alt=""
          width="72"
          height="72"
        />
        <p class="eyebrow">Tide Dial</p>
        <h1 id="product-name">{state.info.productName}</h1>
        <p class="status">Skeleton ready · {state.info.version}</p>
        <p class="detail">Rust core and desktop shell are connected.</p>
      </section>
    </main>
  );
}
