// @vitest-environment jsdom

import "@testing-library/jest-dom/vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/preact";
import { afterEach, describe, expect, it, vi } from "vitest";

import { App, projectLiveFixture, projectRadarFixture } from "./App";
import { saveSettings } from "./api/account-settings";
import { getLiveQuota } from "./api/live-quota";

const { quotaPolicy, appAlertPreferences } = vi.hoisted(() => {
  const alertKinds = [
    "daily_80",
    "daily_100",
    "weekly_remaining_20",
    "weekly_remaining_10",
    "radar_chance_70",
    "quota_reset_confirmed",
    "source_failures_3",
  ] as const;
  return {
    quotaPolicy: {
      policyRevision: 1,
      policyTimezone: "Asia/Shanghai",
      carryWorkdaysEnabled: true,
      baseMicropoints: [
        16_000_000, 16_000_000, 16_000_000, 16_000_000, 16_000_000,
        10_000_000, 10_000_000,
      ],
    },
    appAlertPreferences: alertKinds.flatMap((eventKind) => [
      { eventKind, channel: "system" as const, enabled: true },
      { eventKind, channel: "email" as const, enabled: false },
    ]),
  };
});

function ledgerDay(
  localDate: string,
  usedMicropoints: number | null,
  isToday: boolean,
  status:
    | "unknown"
    | "normal"
    | "warning"
    | "exceeded"
    | "finalized" = "unknown",
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
  getSettings: vi.fn().mockResolvedValue({
    settingsRevision: 1,
    configured: true,
    pathSummary: "…/auth.json",
    accountLabel: "账号 • 21B8",
    notificationPermissionStatus: "granted",
    quotaPolicy,
    alertPreferences: appAlertPreferences,
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
  }),
  onSettingsChanged: vi.fn().mockResolvedValue(vi.fn()),
  saveSettings: vi.fn(),
  sendTestEmail: vi.fn(),
}));

vi.mock("./api/alerts", () => ({
  getAlerts: vi.fn().mockResolvedValue({
    notificationPermissionStatus: "granted",
    events: [],
  }),
  onAlertsChanged: vi.fn().mockResolvedValue(vi.fn()),
  onNotificationOpened: vi.fn().mockResolvedValue(vi.fn()),
  requestSystemNotificationPermission: vi.fn().mockResolvedValue("granted"),
}));

vi.mock("./api/live-quota", () => ({
  getLiveQuota: vi.fn().mockResolvedValue({
    dashboardRevision: 1,
    refreshing: false,
    radar: {
      lastAttemptAtUnixMs: 1_785_000_000_000,
      lastSuccessAtUnixMs: 1_785_000_000_000,
      consecutiveFailures: 0,
      sourceStatus: "fresh",
      publicError: null,
      prediction: null,
      latestAnnouncement: null,
    },
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
      todayBaseMicropoints: 16_000_000,
      todayCarryMicropoints: 0,
      todayLimitMicropoints: 16_000_000,
      todayAvailableMicropoints: 15_000_000,
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

  it("never combines a newly saved account with the previous account quota", async () => {
    vi.mocked(saveSettings).mockResolvedValueOnce({
      settingsRevision: 2,
      configured: true,
      pathSummary: "…/new-auth.json",
      accountLabel: "账号 • 991A",
      notificationPermissionStatus: "granted",
      quotaPolicy,
      alertPreferences: appAlertPreferences,
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
    });
    render(<App />);
    expect(await screen.findByText(/已用 42%/)).toBeInTheDocument();
    vi.mocked(getLiveQuota).mockResolvedValueOnce({
      dashboardRevision: 2,
      refreshing: true,
      quota: null,
      radar: {
        lastAttemptAtUnixMs: null,
        lastSuccessAtUnixMs: null,
        consecutiveFailures: 0,
        sourceStatus: "unavailable",
        publicError: null,
        prediction: null,
        latestAnnouncement: null,
      },
    });

    fireEvent.click(screen.getByRole("button", { name: "打开设置" }));
    fireEvent.input(screen.getByLabelText("auth.json 路径"), {
      target: { value: "/Users/me/.codex/new-auth.json" },
    });
    fireEvent.click(screen.getByRole("button", { name: "保存全部设置" }));

    expect(
      await screen.findByText("账号 • 991A"),
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "取消" }));
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
        todayBaseMicropoints: 16_000_000,
        todayCarryMicropoints: 0,
        todayLimitMicropoints: 16_000_000,
        todayAvailableMicropoints: 15_000_000,
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
    expect(fixture.todayLimit).toBe("基础 16% + 结转 0% = 实际 16%");
  });

  it.each([
    ["warning", "warning"],
    ["exceeded", "over"],
  ] as const)("uses the real current-day %s status for the dashboard tone", (status, tone) => {
    const fixture = projectLiveFixture(
      {
        settingsRevision: 1,
        configured: true,
        pathSummary: "…/auth.json",
        accountLabel: "账号 • 21B8",
        quotaPolicy,
      },
      {
        usedMicropoints: 45_000_000,
        remainingMicropoints: 55_000_000,
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
        todayBaseMicropoints: 16_000_000,
        todayCarryMicropoints: 800_000,
        todayLimitMicropoints: 16_800_000,
        todayAvailableMicropoints: status === "warning" ? 2_600_000 : 0,
        ledgerDays: [
          ledgerDay(
            "2026-07-28",
            status === "warning" ? 14_200_000 : 18_200_000,
            true,
            status,
          ),
        ],
      },
    );

    expect(fixture.tone).toBe(tone);
    expect(fixture.todayLimit).toBe(
      "基础 16% + 结转 0.8% = 实际 16.8%",
    );
  });

  it("formats Rust-projected Radar states without reimplementing expiry policy", () => {
    const active = {
      lastAttemptAtUnixMs: 1_785_000_000_000,
      lastSuccessAtUnixMs: 1_785_000_000_000,
      consecutiveFailures: 0,
      sourceStatus: "fresh" as const,
      publicError: null,
      prediction: {
        chanceBasisPoints: 7_500,
        displayChance: ">70%",
        observedAtUnixMs: 1_784_999_000_000,
        expiresAtUnixMs: 1_785_086_400_000,
        explanation: "Possible additional reset.",
        sourceUrl:
          "https://x.com/thsottiaux/status/2081899343091843463",
      },
      latestAnnouncement: null,
    };

    expect(projectRadarFixture(active)).toMatchObject({
      kind: "active",
      chance: ">70%",
      sourceUrl:
        "https://x.com/thsottiaux/status/2081899343091843463",
    });
    expect(
      projectRadarFixture(active),
    ).toMatchObject({
      kind: "active",
      chance: ">70%",
    });
    expect(
      projectRadarFixture({
          ...active,
          sourceStatus: "stale_after_failure",
          publicError: "timeout",
          prediction: null,
          consecutiveFailures: 1,
        }),
    ).toMatchObject({
      kind: "empty",
      message: "预测数据暂不可用",
    });
  });

  it("keeps Radar visible while the Codex account is unconfigured", () => {
    const fixture = projectLiveFixture(
      {
        settingsRevision: 0,
        configured: false,
        pathSummary: null,
        accountLabel: null,
        quotaPolicy,
      },
      null,
      1_785_000_000_000,
      {
        lastAttemptAtUnixMs: 1_785_000_000_000,
        lastSuccessAtUnixMs: 1_785_000_000_000,
        consecutiveFailures: 0,
        sourceStatus: "fresh",
        publicError: null,
        prediction: {
          chanceBasisPoints: 7_500,
          displayChance: ">70%",
          observedAtUnixMs: 1_784_999_000_000,
          expiresAtUnixMs: 1_785_086_400_000,
          explanation: "第三方预测",
          sourceUrl:
            "https://x.com/thsottiaux/status/2081899343091843463",
        },
        latestAnnouncement: null,
      },
    );

    expect(fixture.tone).toBe("unconfigured");
    expect(fixture.radar).toMatchObject({ kind: "active", chance: ">70%" });
  });
});
