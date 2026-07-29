// @vitest-environment jsdom

import "@testing-library/jest-dom/vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/preact";
import { afterEach, describe, expect, it, vi } from "vitest";

import { App, projectLiveFixture } from "./App";
import { selectAuthFile } from "./api/account-settings";
import { getLiveQuota } from "./api/live-quota";

const { quotaPolicy } = vi.hoisted(() => ({
  quotaPolicy: {
    policyRevision: 1,
    policyTimezone: "Asia/Shanghai",
    carryWorkdaysEnabled: true,
    baseMicropoints: [
      16_000_000, 16_000_000, 16_000_000, 16_000_000, 16_000_000,
      10_000_000, 10_000_000,
    ],
  },
}));

function ledgerDay(
  localDate: string,
  usedMicropoints: number | null,
  isToday: boolean,
  status: "unknown" | "normal" | "finalized" = "unknown",
) {
  return {
    localDate,
    usedMicropoints,
    policyRevision: 1,
    policyTimezone: "Asia/Shanghai",
    baseMicropoints: 16_000_000,
    carryMicropoints: 0,
    limitMicropoints: 16_000_000,
    isToday,
    finalized: status === "finalized",
    status,
  };
}

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
    quotaPolicy,
  }),
  selectAuthFile: vi.fn(),
}));

vi.mock("./api/live-quota", () => ({
  getLiveQuota: vi.fn().mockResolvedValue({
    dashboardRevision: 1,
    refreshing: false,
    quota: {
      usedMicropoints: 42_000_000,
      remainingMicropoints: 58_000_000,
      capturedAtUnixMs: 1_785_000_000_000,
      resetsAtUnixS: 1_786_000_000,
      windowStartsAtUnixS: 1_785_395_200,
      windowEndsAtUnixS: 1_785_999_999,
      planType: "plus",
      allowed: true,
      lastAttemptAtUnixMs: 1_785_000_000_000,
      lastSuccessAtUnixMs: 1_785_000_000_000,
      consecutiveFailures: 0,
      sourceStatus: "fresh",
      publicError: null,
      ledgerDays: [
        ledgerDay("2026-07-24", null, false),
        ledgerDay("2026-07-25", null, false),
        ledgerDay("2026-07-26", null, false),
        ledgerDay("2026-07-27", null, false),
        ledgerDay("2026-07-28", 1_000_000, true, "normal"),
        ledgerDay("2026-07-29", null, false),
        ledgerDay("2026-07-30", null, false),
      ],
    },
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

  it("never combines a newly selected account with the previous account quota", async () => {
    vi.mocked(selectAuthFile).mockResolvedValueOnce({
      settingsRevision: 2,
      configured: true,
      pathSummary: "…/new-auth.json",
      accountLabel: "账号 • 991A",
      quotaPolicy,
    });
    render(<App />);
    expect(await screen.findByText(/已用 42%/)).toBeInTheDocument();
    vi.mocked(getLiveQuota).mockResolvedValueOnce({
      dashboardRevision: 2,
      refreshing: true,
      quota: null,
    });

    fireEvent.click(screen.getByRole("button", { name: "打开设置" }));
    fireEvent.click(screen.getByRole("button", { name: "更换 auth.json" }));

    expect(
      await screen.findByText("账号 • 991A"),
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "完成" }));
    expect(screen.getByText("Codex 额度 · 正在刷新")).toBeInTheDocument();
    expect(screen.queryByText(/已用 42%/)).not.toBeInTheDocument();
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
        quotaPolicy,
      },
      {
        usedMicropoints: 42_000_000,
        remainingMicropoints: 58_000_000,
        capturedAtUnixMs: 1_785_000_000_000,
        resetsAtUnixS: 1_786_000_000,
        windowStartsAtUnixS: 1_785_395_200,
        windowEndsAtUnixS: 1_785_999_999,
        planType: "plus",
        allowed: true,
        lastAttemptAtUnixMs: 1_785_003_600_000,
        lastSuccessAtUnixMs: 1_785_000_000_000,
        consecutiveFailures: 1,
        sourceStatus: "stale_after_failure",
        publicError: "timeout",
        ledgerDays: [
          ledgerDay("2026-07-24", null, false),
          ledgerDay("2026-07-25", null, false),
          ledgerDay("2026-07-26", null, false),
          ledgerDay("2026-07-27", null, false),
          ledgerDay("2026-07-28", 1_000_000, true, "normal"),
          ledgerDay("2026-07-29", null, false),
          ledgerDay("2026-07-30", null, false),
        ],
      },
      1_785_003_600_000,
    );

    expect(fixture.weeklyUsed).toBe("42%");
    expect(fixture.days).toHaveLength(7);
    expect(fixture.days[4]).toMatchObject({
      label: "今天",
      date: "07/28",
      used: 1,
      limit: 16,
      status: "进行中",
    });
    expect(fixture.sourceHealth).toBe("Codex 额度 · 连续 1 次失败（请求超时）");
    expect(fixture.lastSuccess).toContain("上次成功");
  });
});
