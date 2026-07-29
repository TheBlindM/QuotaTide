// @vitest-environment jsdom

import "@testing-library/jest-dom/vitest";
import { cleanup, render, screen } from "@testing-library/preact";
import { afterEach, describe, expect, it, vi } from "vitest";

import { App, projectLiveFixture } from "./App";

vi.mock("./api/build-info", () => ({
  loadBuildInfo: vi.fn().mockResolvedValue({
    productName: "QuotaTide",
    version: "0.1.0",
    author: "TheBlind",
    identifier: "dev.theblind.quotatide",
    stage: "skeleton",
  }),
}));

vi.mock("./api/account-settings", () => ({
  getAccountSettings: vi.fn().mockResolvedValue({
    settingsRevision: 1,
    configured: true,
    pathSummary: "…/auth.json",
    accountLabel: "账号 • 21B8",
  }),
  selectAuthFile: vi.fn(),
}));

vi.mock("./api/live-quota", () => ({
  getLiveQuota: vi.fn().mockResolvedValue({
    usedMicropoints: 42_000_000,
    remainingMicropoints: 58_000_000,
    capturedAtUnixMs: 1_785_000_000_000,
    resetsAtUnixS: 1_786_000_000,
    planType: "plus",
    allowed: true,
    lastAttemptAtUnixMs: 1_785_000_000_000,
    lastSuccessAtUnixMs: 1_785_000_000_000,
    consecutiveFailures: 0,
    freshness: "fresh",
    publicError: null,
  }),
  onDashboardChanged: vi.fn().mockResolvedValue(vi.fn()),
}));

afterEach(() => {
  cleanup();
  window.history.replaceState({}, "", "/");
  delete document.documentElement.dataset.theme;
  delete document.documentElement.dataset.surface;
  delete document.documentElement.dataset.platformFallback;
});

describe("QuotaTide tray app", () => {
  it("waits for the Rust shell and then shows the weekly ledger", async () => {
    render(<App />);

    expect(
      await screen.findByRole("heading", { name: "QuotaTide" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("table", { name: /当前七日窗口/ }),
    ).toBeInTheDocument();
    expect(screen.getByText(/已用 42%/)).toBeInTheDocument();
  });

  it("provides deterministic dark and opaque visual fallbacks", () => {
    window.history.replaceState(
      {},
      "",
      "/?preview&state=warning&theme=dark&surface=opaque",
    );

    render(<App />);

    expect(document.documentElement).toHaveAttribute("data-theme", "dark");
    expect(document.documentElement).toHaveAttribute("data-surface", "opaque");
    expect(screen.getByRole("alert")).toHaveTextContent("接近今日额度");
  });

  it("does not overwrite a native opaque fallback during startup", () => {
    document.documentElement.dataset.surface = "opaque";
    document.documentElement.dataset.platformFallback = "true";
    window.history.replaceState({}, "", "/?preview");

    render(<App />);

    expect(document.documentElement).toHaveAttribute("data-surface", "opaque");
  });

  it("projects a safe current error alongside the last successful quota", () => {
    const fixture = projectLiveFixture(
      {
        settingsRevision: 1,
        configured: true,
        pathSummary: "…/auth.json",
        accountLabel: "账号 • 21B8",
      },
      {
        usedMicropoints: 42_000_000,
        remainingMicropoints: 58_000_000,
        capturedAtUnixMs: 1_785_000_000_000,
        resetsAtUnixS: 1_786_000_000,
        planType: "plus",
        allowed: true,
        lastAttemptAtUnixMs: 1_785_003_600_000,
        lastSuccessAtUnixMs: 1_785_000_000_000,
        consecutiveFailures: 1,
        freshness: "stale",
        publicError: "timeout",
      },
      1_785_003_600_000,
    );

    expect(fixture.weeklyUsed).toBe("42%");
    expect(fixture.sourceHealth).toBe("Codex 额度 · 连续 1 次失败（请求超时）");
    expect(fixture.lastSuccess).toContain("上次成功");
  });
});
