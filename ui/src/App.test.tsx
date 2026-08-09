// @vitest-environment jsdom

import "@testing-library/jest-dom/vitest";
import {
  cleanup,
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/preact";
import { afterEach, describe, expect, it, vi } from "vitest";

import { App, projectLiveFixture, projectRadarFixture } from "./App";
import { saveSettings } from "./api/account-settings";
import { getLiveQuota, onDashboardChanged } from "./api/live-quota";
import {
  getStartupState,
  openLocalDataDirectory,
  retryLocalRecovery,
} from "./api/local-data";
import { setAccessibleSurface } from "./api/tray-shell";

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
    suggestedLimitMicropoints: null,
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
    autoUpdateEnabled: true,
    trayDisplayMode: "wave",
    storyTheme: "rising_water",
    interfaceLocale: "zh-CN",
    formatLocale: "zh-CN",
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
  pickAuthFile: vi.fn().mockResolvedValue(null),
  saveSettings: vi.fn(),
  sendTestEmail: vi.fn(),
}));

vi.mock("./api/alerts", () => ({
  dismissAlert: vi.fn(),
  dismissAllAlerts: vi.fn(),
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
      pressure: "safe",
      burnProjection: null,
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
      todayAvailabilityKind: "actual",
      ledgerDays: [
        ledgerDay("2026-07-24", null, false),
        ledgerDay("2026-07-25", null, false),
        ledgerDay("2026-07-26", null, false),
        ledgerDay("2026-07-27", null, false),
        ledgerDay("2026-07-28", 1_000_000, true, "normal"),
        ledgerDay("2026-07-29", null, false),
        ledgerDay("2026-07-30", null, false),
        ledgerDay("2026-07-31", null, false),
      ],
    },
  }),
  onDashboardChanged: vi.fn().mockResolvedValue(vi.fn()),
}));

vi.mock("./api/reset-credits", () => ({
  getResetCredits: vi.fn().mockResolvedValue({
    availableCount: 0,
    credits: [],
    checkedAtUnixMs: 1_785_000_000_000,
  }),
}));

vi.mock("./api/local-data", () => ({
  getStartupState: vi.fn().mockResolvedValue({
    mode: "ready",
    messageKey: "startup.ready",
    recoveredFromBackup: false,
  }),
  openLocalDataDirectory: vi.fn().mockResolvedValue(undefined),
  retryLocalRecovery: vi.fn().mockResolvedValue(undefined),
  exportDiagnostics: vi.fn().mockResolvedValue(true),
  clearAllLocalData: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("./api/tray-shell", () => ({
  hideMainWindow: vi.fn().mockResolvedValue(undefined),
  requestManualRefresh: vi.fn().mockResolvedValue(0),
  setAccessibleSurface: vi.fn().mockResolvedValue(true),
  setMainWindowExpanded: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("./api/updater", () => ({
  getUpdateState: vi.fn().mockResolvedValue({
    status: "idle",
    currentVersion: "0.1.0",
    availableVersion: null,
    notes: null,
    lastCheckedAtUnixMs: null,
    errorCode: null,
  }),
  requestUpdateCheck: vi.fn(),
  installPendingUpdate: vi.fn(),
  onUpdateState: vi.fn().mockResolvedValue(vi.fn()),
}));

afterEach(() => {
  vi.useRealTimers();
  cleanup();
  vi.clearAllMocks();
  vi.mocked(getStartupState).mockResolvedValue({
    mode: "ready",
    messageKey: "startup.ready",
    recoveredFromBackup: false,
  });
  window.history.replaceState({}, "", "/");
  delete document.documentElement.dataset.runtime;
  delete document.documentElement.dataset.theme;
  delete document.documentElement.dataset.surface;
  delete document.documentElement.dataset.platformFallback;
  Reflect.deleteProperty(window, "matchMedia");
});

describe("QuotaTide tray app", () => {
  it("waits for the Rust shell and then shows the weekly ledger", async () => {
    render(<App />);

    expect(
      await screen.findByRole("heading", { name: "QuotaTide" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("list", { name: /本周策略/ })).toBeInTheDocument();
    expect(
      screen.getByRole("group", { name: /已用 42%，剩余 58%/ }),
    ).toBeInTheDocument();
  });

  it("reconciles a refresh even while dashboard listeners are still attaching", async () => {
    const steadyState = await getLiveQuota();
    vi.mocked(getLiveQuota).mockClear();
    vi.mocked(getLiveQuota)
      .mockResolvedValueOnce({ ...steadyState, refreshing: true })
      .mockResolvedValueOnce({ ...steadyState, refreshing: false });
    vi.mocked(onDashboardChanged).mockReturnValueOnce(
      new Promise(() => undefined),
    );
    vi.useFakeTimers({ shouldAdvanceTime: true });

    render(<App />);

    expect(
      await screen.findByText("Codex 额度 · 正在刷新"),
    ).toBeInTheDocument();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1_100);
    });
    await waitFor(() => {
      expect(
        screen.getByRole("button", { name: "立即刷新" }),
      ).toBeEnabled();
    });
    expect(getLiveQuota).toHaveBeenCalledTimes(2);
  });

  it("reprojects the current ledger day when policy midnight passes", async () => {
    const steadyState = await getLiveQuota();
    const quota = steadyState.quota;
    if (quota === null) {
      throw new Error("Expected the default live quota fixture");
    }
    const beforeMidnight = {
      ...steadyState,
      quota: {
        ...quota,
        ledgerDays: quota.ledgerDays.map((day) => ({
          ...day,
          isToday: day.localDate === "2026-07-28",
        })),
      },
    };
    const afterMidnight = {
      ...steadyState,
      quota: {
        ...quota,
        ledgerDays: quota.ledgerDays.map((day) => ({
          ...day,
          isToday: day.localDate === "2026-07-29",
        })),
      },
    };
    vi.mocked(getLiveQuota).mockClear();
    vi.mocked(getLiveQuota)
      .mockResolvedValueOnce(beforeMidnight)
      .mockResolvedValueOnce(beforeMidnight)
      .mockResolvedValueOnce(afterMidnight);
    vi.useFakeTimers({ shouldAdvanceTime: true });
    vi.setSystemTime(new Date("2026-07-28T15:59:30.000Z"));

    render(<App />);

    await waitFor(() => {
      expect(document.querySelector(".is-today")).toHaveTextContent("07/28");
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(31_000);
    });
    await waitFor(() => {
      expect(document.querySelector(".is-today")).toHaveTextContent("07/29");
    });
    expect(getLiveQuota).toHaveBeenCalledTimes(3);
  });

  it("keeps a visible notice after an automatic validated-backup recovery", async () => {
    vi.mocked(getStartupState).mockResolvedValueOnce({
      mode: "ready",
      messageKey: "startup.ready",
      recoveredFromBackup: true,
    });

    render(<App />);

    expect(
      await screen.findByText(/已从最近的有效备份恢复本地账本/),
    ).toBeInTheDocument();
  });

  it("keeps recovery actions available when the local database cannot open", async () => {
    vi.mocked(getStartupState).mockResolvedValueOnce({
      mode: "recovery_required",
      messageKey: "startup.recovery_required",
      recoveredFromBackup: false,
    });

    render(<App />);

    expect(
      await screen.findByRole("heading", { name: "本地账本需要处理" }),
    ).toBeInTheDocument();
    expect(screen.getByText(/没有创建空账本/)).toBeInTheDocument();
    expect(screen.getByText(/auth.json 不在处理范围内/)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "重试恢复" }));
    expect(retryLocalRecovery).toHaveBeenCalledOnce();
    const openData = screen.getByRole("button", { name: "打开数据目录" });
    await waitFor(() => expect(openData).toBeEnabled());
    fireEvent.click(openData);
    expect(openLocalDataDirectory).toHaveBeenCalledOnce();
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
      autoUpdateEnabled: true,
      trayDisplayMode: "wave",
      storyTheme: "rising_water",
      interfaceLocale: "zh-CN",
      formatLocale: "zh-CN",
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
    expect(
      await screen.findByRole("group", { name: /已用 42%，剩余 58%/ }),
    ).toBeInTheDocument();
    await waitFor(() => {
      expect(getLiveQuota).toHaveBeenCalledTimes(2);
    });
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

    fireEvent.click(screen.getByRole("button", { name: "设置" }));
    fireEvent.input(screen.getByLabelText("auth.json 路径"), {
      target: { value: "/Users/me/.codex/new-auth.json" },
    });
    fireEvent.click(screen.getByRole("button", { name: "保存全部设置" }));

    expect(
      await screen.findByText("账号 • 991A"),
    ).toBeInTheDocument();
    const cancel = await screen.findByRole("button", { name: "取消" });
    fireEvent.click(cancel);
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
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(screen.getByLabelText(/今天还可用 2\.6%/u)).toHaveClass(
      "side-stat--warning",
    );
  });

  it("renders app-owned English preview copy from structured quota data", () => {
    window.history.replaceState(
      {},
      "",
      "/?preview&state=warning&radar=active&lang=en&format=en-US",
    );

    render(<App />);

    expect(screen.getByText("Weekly remaining")).toBeInTheDocument();
    expect(
      screen.getByRole("group", { name: /45% used, 55% remaining/ }),
    ).toBeInTheDocument();
    expect(screen.getByText("I'm feeling like a limit reset.")).toBeInTheDocument();
    expect(screen.getByText("Codex limits were reset.")).toBeInTheDocument();
    expect(document.body).toHaveAttribute(
      "data-platform-fallback-message",
      "System glass is unavailable; opaque mode is active",
    );
  });

  it("keeps interface language independent from the format locale", () => {
    window.history.replaceState(
      {},
      "",
      "/?preview&state=warning&lang=en&format=zh-CN",
    );

    render(<App />);

    expect(screen.getByText("Weekly remaining")).toBeInTheDocument();
    expect(screen.getAllByTitle(/2026年7月31日/u)).toHaveLength(2);
    expect(screen.getAllByText("周五")).toHaveLength(4);
    expect(screen.queryByText("Fri")).not.toBeInTheDocument();
  });

  it("does not overwrite a native opaque fallback during startup", () => {
    document.documentElement.dataset.surface = "opaque";
    document.documentElement.dataset.platformFallback = "true";
    window.history.replaceState({}, "", "/?preview");

    render(<App />);

    expect(document.documentElement).toHaveAttribute("data-surface", "opaque");
  });

  it("keeps the native window material synchronized with accessibility display changes", async () => {
    const listeners = new Set<() => void>();
    const queries = [
      { matches: false },
      { matches: false },
      { matches: false },
    ].map((state) => ({
      ...state,
      media: "",
      onchange: null,
      addEventListener: (_name: string, listener: () => void) =>
        listeners.add(listener),
      removeEventListener: (_name: string, listener: () => void) =>
        listeners.delete(listener),
      addListener: vi.fn(),
      removeListener: vi.fn(),
      dispatchEvent: vi.fn(),
    }));
    const matchMedia = vi
      .fn()
      .mockImplementation(() => queries.shift() ?? queries[0]);
    window.matchMedia = matchMedia;

    render(<App />);

    await waitFor(() => {
      expect(setAccessibleSurface).toHaveBeenCalledWith(false);
    });
    const reducedTransparency = matchMedia.mock.results[0]
      ?.value as MediaQueryList;
    Object.defineProperty(reducedTransparency, "matches", {
      configurable: true,
      value: true,
    });
    for (const listener of listeners) {
      listener();
    }

    await waitFor(() => {
      expect(setAccessibleSurface).toHaveBeenLastCalledWith(true);
    });
    expect(document.documentElement).toHaveAttribute(
      "data-surface",
      "opaque",
    );
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
        pressure: "safe",
        burnProjection: null,
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
        todayAvailabilityKind: "actual",
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

  it("projects a mid-window baseline as suggested quota from now", async () => {
    const state = await getLiveQuota();
    const quota = state.quota;
    if (quota === null) {
      throw new Error("Expected the default live quota fixture");
    }
    const fixture = projectLiveFixture(
      {
        settingsRevision: 1,
        configured: true,
        pathSummary: "…/auth.json",
        accountLabel: "账号 • 21B8",
        quotaPolicy,
      },
      {
        ...quota,
        usedMicropoints: 48_000_000,
        remainingMicropoints: 52_000_000,
        todayAvailableMicropoints: 13_000_000,
        todayAvailabilityKind: "suggested_from_now",
        ledgerDays: quota.ledgerDays.map((day, index) => ({
          ...day,
          usedMicropoints: null,
          suggestedLimitMicropoints:
            index < 4
              ? null
              : [13_000_000, 13_000_000, 13_000_000, 13_000_000][index - 4],
          status: "unknown" as const,
        })),
      },
    );

    expect(fixture.todayAvailable).toBe("13%");
    expect(fixture.todayAvailabilityKind).toBe("suggested_from_now");
    expect(fixture.days.find((day) => day.today)).toMatchObject({
      used: null,
      suggested: 13,
    });
    expect(
      fixture.days.filter((day) => day.suggested !== null).map((day) => day.suggested),
    ).toEqual([13, 13, 13, 13]);
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
        pressure: "safe",
        burnProjection: null,
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
        todayAvailabilityKind: "actual",
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
      message: "重置数据暂不可用",
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
