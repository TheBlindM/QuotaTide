// @vitest-environment jsdom

import "@testing-library/jest-dom/vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/preact";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { AlertEventKind } from "./bindings/AlertEventKind";
import type { PublicSettings } from "./bindings/PublicSettings";
import type { SettingsDraft } from "./bindings/SettingsDraft";
import { TrayApp } from "./TrayApp";
import { ledgerFixtures } from "./WeeklyLedger";

afterEach(cleanup);

const quotaPolicy = {
  policyRevision: 1,
  policyTimezone: "Asia/Shanghai",
  carryWorkdaysEnabled: true,
  baseMicropoints: [
    16_000_000, 16_000_000, 16_000_000, 16_000_000, 16_000_000,
    10_000_000, 10_000_000,
  ],
};

const alertKinds: AlertEventKind[] = [
  "daily_80",
  "daily_100",
  "weekly_remaining_20",
  "weekly_remaining_10",
  "radar_chance_70",
  "quota_reset_confirmed",
  "source_failures_3",
];

const alertPreferences = alertKinds.flatMap((eventKind) => [
  { eventKind, channel: "system" as const, enabled: true },
  { eventKind, channel: "email" as const, enabled: false },
]);

const atomicSettings: PublicSettings = {
  settingsRevision: 4,
  configured: true,
  pathSummary: "…/auth.json",
  accountLabel: "账号 • 21B8",
  notificationPermissionStatus: "granted",
  quotaPolicy,
  alertPreferences,
  autostartEnabled: false,
};

describe("tray-window navigation", () => {
  it("saves account policy alerts and autostart as one revisioned draft", async () => {
    const onSaveSettings = vi
      .fn<(draft: SettingsDraft) => Promise<PublicSettings>>()
      .mockResolvedValue({
        ...atomicSettings,
        settingsRevision: 5,
        autostartEnabled: true,
      });
    render(
      <TrayApp
        fixture={ledgerFixtures.fresh}
        settings={atomicSettings}
        onHide={vi.fn()}
        onRefresh={vi.fn()}
        onSaveSettings={onSaveSettings}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "打开设置" }));
    fireEvent.input(screen.getByLabelText("auth.json 路径"), {
      target: { value: "/Users/me/.codex/auth.json" },
    });
    fireEvent.click(screen.getByLabelText("登录后自动启动"));
    fireEvent.click(screen.getByRole("tab", { name: "额度" }));
    fireEvent.input(screen.getByLabelText("周一额度"), {
      target: { value: "15" },
    });
    fireEvent.click(screen.getByRole("tab", { name: "提醒" }));
    fireEvent.click(screen.getByLabelText("每日额度达到 80% 邮件提醒"));
    fireEvent.click(screen.getByRole("button", { name: "保存全部设置" }));

    await waitFor(() => {
      expect(onSaveSettings).toHaveBeenCalledWith({
        expectedSettingsRevision: 4,
        authPath: "/Users/me/.codex/auth.json",
        quotaPolicy: {
          policyTimezone: "Asia/Shanghai",
          carryWorkdaysEnabled: true,
          baseMicropoints: [
            15_000_000, 16_000_000, 16_000_000, 16_000_000, 16_000_000,
            10_000_000, 10_000_000,
          ],
        },
        alertPreferences: alertPreferences.map((preference) =>
          preference.eventKind === "daily_80" &&
          preference.channel === "email"
            ? { ...preference, enabled: true }
            : preference,
        ),
        autostartEnabled: true,
      });
    });
  });

  it("opens settings and returns to the weekly ledger", () => {
    render(
      <TrayApp
        fixture={ledgerFixtures.fresh}
        settings={{ ...atomicSettings, configured: false, accountLabel: null }}
        onHide={vi.fn()}
        onRefresh={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "打开设置" }));
    expect(screen.getByRole("heading", { name: "设置" })).toBeInTheDocument();
    expect(screen.getByText("尚未配置 Codex 账号")).toBeInTheDocument();
    expect(screen.getByLabelText("auth.json 路径")).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "额度" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "账号" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expect(screen.getByRole("tab", { name: "提醒" })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "取消" }));
    expect(
      screen.getByRole("table", { name: /当前七日窗口/ }),
    ).toBeInTheDocument();
  });

  it("leaves settings and opens the target when a notification is activated", async () => {
    const { rerender } = render(
      <TrayApp
        fixture={ledgerFixtures.fresh}
        settings={atomicSettings}
        focusRequest={null}
        onHide={vi.fn()}
        onRefresh={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "打开设置" }));
    expect(screen.getByRole("heading", { name: "设置" })).toBeInTheDocument();

    rerender(
      <TrayApp
        fixture={ledgerFixtures.fresh}
        settings={atomicSettings}
        focusRequest={{ target: "radar", activationId: 1 }}
        onHide={vi.fn()}
        onRefresh={vi.fn()}
      />,
    );

    await waitFor(() => {
      expect(
        screen.getByRole("region", { name: "重置雷达" }),
      ).toHaveFocus();
    });
  });

  it("requests notification permission only from the explicit alerts action", () => {
    const onRequestNotificationPermission = vi.fn().mockResolvedValue("granted");
    render(
      <TrayApp
        fixture={ledgerFixtures.fresh}
        settings={{
          ...atomicSettings,
          notificationPermissionStatus: "unknown",
        }}
        onHide={vi.fn()}
        onRefresh={vi.fn()}
        onRequestNotificationPermission={onRequestNotificationPermission}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "打开设置" }));
    fireEvent.click(screen.getByRole("tab", { name: "提醒" }));
    fireEvent.click(
      screen.getByRole("button", { name: "启用系统通知" }),
    );

    expect(onRequestNotificationPermission).toHaveBeenCalledOnce();
  });

  it("lets a denied permission be checked again without hiding in-app alerts", () => {
    const onRequestNotificationPermission = vi.fn().mockResolvedValue("denied");
    render(
      <TrayApp
        fixture={ledgerFixtures.fresh}
        settings={{
          ...atomicSettings,
          notificationPermissionStatus: "denied",
        }}
        onHide={vi.fn()}
        onRefresh={vi.fn()}
        onRequestNotificationPermission={onRequestNotificationPermission}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "打开设置" }));
    fireEvent.click(screen.getByRole("tab", { name: "提醒" }));
    fireEvent.click(screen.getByRole("button", { name: "重新检查权限" }));

    expect(onRequestNotificationPermission).toHaveBeenCalledOnce();
  });

  it("reloads the public revision after an atomic settings conflict", async () => {
    const onSaveSettings = vi.fn().mockRejectedValue({
      code: "settings_conflict",
      messageKey: "settings.revision_conflict",
      safeContext: { maxBytes: null },
    });
    const onReloadSettings = vi.fn().mockResolvedValue({
      ...atomicSettings,
      settingsRevision: 5,
      accountLabel: "账号 • 55AA",
    });
    render(
      <TrayApp
        fixture={ledgerFixtures.fresh}
        settings={atomicSettings}
        onHide={vi.fn()}
        onRefresh={vi.fn()}
        onSaveSettings={onSaveSettings}
        onReloadSettings={onReloadSettings}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "打开设置" }));
    fireEvent.click(screen.getByRole("button", { name: "保存全部设置" }));

    expect(await screen.findByText("账号 • 55AA")).toBeInTheDocument();
    expect(onReloadSettings).toHaveBeenCalledOnce();
    expect(screen.getByRole("alert")).toHaveTextContent("设置未保存");
  });

  it("preserves unsaved edits when background refresh keeps the same revision", () => {
    const { rerender } = render(
      <TrayApp
        fixture={ledgerFixtures.fresh}
        settings={atomicSettings}
        onHide={vi.fn()}
        onRefresh={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "打开设置" }));
    fireEvent.click(screen.getByRole("tab", { name: "额度" }));
    fireEvent.input(screen.getByLabelText("周一额度"), {
      target: { value: "15" },
    });

    rerender(
      <TrayApp
        fixture={ledgerFixtures.fresh}
        settings={{ ...atomicSettings }}
        onHide={vi.fn()}
        onRefresh={vi.fn()}
      />,
    );

    expect(screen.getByLabelText("周一额度")).toHaveValue(15);
  });

  it("supports platform keyboard shortcuts without opening another window", () => {
    const onHide = vi.fn();
    const onRefresh = vi.fn();
    render(
      <TrayApp
        fixture={ledgerFixtures.fresh}
        onHide={onHide}
        onRefresh={onRefresh}
      />,
    );

    fireEvent.keyDown(window, { key: ",", metaKey: true });
    expect(screen.getByRole("heading", { name: "设置" })).toBeInTheDocument();

    fireEvent.keyDown(window, { key: "Escape" });
    expect(screen.getByRole("heading", { name: "QuotaTide" })).toBeInTheDocument();

    fireEvent.keyDown(window, { key: "r", ctrlKey: true });
    expect(onRefresh).toHaveBeenCalledOnce();

    fireEvent.keyDown(window, { key: "Escape" });
    expect(onHide).toHaveBeenCalledOnce();
  });

  it("keeps data visible and coalesces refreshes while one is running", () => {
    let completeRefresh: (() => void) | undefined;
    const onRefresh = vi.fn(
      () =>
        new Promise<void>((resolve) => {
          completeRefresh = resolve;
        }),
    );
    render(
      <TrayApp
        fixture={ledgerFixtures.fresh}
        onHide={vi.fn()}
        onRefresh={onRefresh}
      />,
    );

    const refresh = screen.getByRole("button", { name: "立即刷新" });
    fireEvent.click(refresh);
    fireEvent.click(refresh);

    expect(onRefresh).toHaveBeenCalledOnce();
    expect(screen.getByRole("button", { name: "正在刷新" })).toBeDisabled();
    expect(
      screen.getByRole("table", { name: /当前七日窗口/ }),
    ).toBeInTheDocument();

    completeRefresh?.();
  });

  it("shows scheduler and settings refresh activity reported by the native shell", () => {
    const onRefresh = vi.fn();
    render(
      <TrayApp
        fixture={ledgerFixtures.fresh}
        externalRefreshing
        onHide={vi.fn()}
        onRefresh={onRefresh}
      />,
    );

    expect(screen.getByRole("button", { name: "正在刷新" })).toBeDisabled();
    expect(screen.getByText("Codex 额度 · 正在刷新")).toBeInTheDocument();
    fireEvent.keyDown(window, { key: "r", ctrlKey: true });
    expect(onRefresh).not.toHaveBeenCalled();
  });
});
