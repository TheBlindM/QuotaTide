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

describe("tray-window navigation", () => {
  it("opens settings and returns to the weekly ledger", () => {
    render(
      <TrayApp
        fixture={ledgerFixtures.fresh}
        onHide={vi.fn()}
        onRefresh={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "打开设置" }));
    expect(screen.getByRole("heading", { name: "设置" })).toBeInTheDocument();
    expect(screen.getByText("尚未配置 Codex 账号")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "选择 auth.json" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "额度" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "账号" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expect(screen.getByRole("tab", { name: "通知" })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "完成" }));
    expect(
      screen.getByRole("table", { name: /当前七日窗口/ }),
    ).toBeInTheDocument();
  });

  it("selects auth.json through the native command and only renders safe account data", async () => {
    const onSelectAuth = vi.fn().mockResolvedValue({
      settingsRevision: 1,
      configured: true,
      pathSummary: "…/auth.json",
      accountLabel: "账号 • 9A2F",
      quotaPolicy,
    });
    render(
      <TrayApp
        fixture={ledgerFixtures.unconfigured}
        accountSettings={{
          settingsRevision: 0,
          configured: false,
          pathSummary: null,
          accountLabel: null,
          quotaPolicy,
        }}
        onHide={vi.fn()}
        onRefresh={vi.fn()}
        onSelectAuth={onSelectAuth}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "选择 auth.json" }));
    fireEvent.click(screen.getByRole("button", { name: "选择 auth.json" }));

    expect(await screen.findByText("账号 • 9A2F")).toBeInTheDocument();
    expect(screen.getByText("…/auth.json")).toBeInTheDocument();
    expect(onSelectAuth).toHaveBeenCalledExactlyOnceWith(0);
  });

  it("keeps the previous account projection when native validation fails", async () => {
    const canaries = {
      accessToken: "access-ticket16-command-canary",
      accountId: "account-ticket16-command-canary",
      idToken: "jwt-ticket16-command-canary",
    };
    const onSelectAuth = vi.fn().mockRejectedValue({
      code: "auth_invalid_json",
      messageKey: "auth.format.invalid_json",
      nested: canaries,
    });
    render(
      <TrayApp
        fixture={ledgerFixtures.fresh}
        accountSettings={{
          settingsRevision: 4,
          configured: true,
          pathSummary: "…/auth.json",
          accountLabel: "账号 • 21B8",
          quotaPolicy,
        }}
        onHide={vi.fn()}
        onRefresh={vi.fn()}
        onSelectAuth={onSelectAuth}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "打开设置" }));
    fireEvent.click(screen.getByRole("button", { name: "更换 auth.json" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "无法验证该文件",
    );
    expect(screen.getByText("账号 • 21B8")).toBeInTheDocument();
    expect(document.body).not.toHaveTextContent("auth.format.invalid_json");
    for (const canary of Object.values(canaries)) {
      expect(document.body).not.toHaveTextContent(canary);
    }
  });

  it("reloads the public revision after a settings conflict before retrying", async () => {
    const onSelectAuth = vi
      .fn()
      .mockRejectedValueOnce({
        code: "settings_conflict",
        messageKey: "settings.revision_conflict",
        safeContext: { maxBytes: null },
      })
      .mockResolvedValueOnce({
        settingsRevision: 6,
        configured: true,
        pathSummary: "…/auth.json",
        accountLabel: "账号 • 66AA",
        quotaPolicy,
      });
    const onReloadAccount = vi.fn().mockResolvedValue({
      settingsRevision: 5,
      configured: true,
      pathSummary: "…/auth.json",
      accountLabel: "账号 • 55AA",
      quotaPolicy,
    });
    render(
      <TrayApp
        fixture={ledgerFixtures.fresh}
        accountSettings={{
          settingsRevision: 4,
          configured: true,
          pathSummary: "…/auth.json",
          accountLabel: "账号 • 44AA",
          quotaPolicy,
        }}
        onHide={vi.fn()}
        onRefresh={vi.fn()}
        onSelectAuth={onSelectAuth}
        onReloadAccount={onReloadAccount}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "打开设置" }));
    fireEvent.click(screen.getByRole("button", { name: "更换 auth.json" }));
    expect(await screen.findByText("账号 • 55AA")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "更换 auth.json" }));
    expect(await screen.findByText("账号 • 66AA")).toBeInTheDocument();
    expect(onSelectAuth).toHaveBeenNthCalledWith(1, 4);
    expect(onSelectAuth).toHaveBeenNthCalledWith(2, 5);
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

  it("validates and saves a complete seven-day quota policy", async () => {
    const onUpdatePolicy = vi.fn().mockResolvedValue({
      settingsRevision: 1,
      configured: false,
      pathSummary: null,
      accountLabel: null,
      quotaPolicy: {
        ...quotaPolicy,
        policyRevision: 2,
        baseMicropoints: [
          15_000_000, 16_000_000, 16_000_000, 16_000_000, 16_000_000,
          10_000_000, 10_000_000,
        ],
      },
    });
    render(
      <TrayApp
        fixture={ledgerFixtures.fresh}
        accountSettings={{
          settingsRevision: 0,
          configured: false,
          pathSummary: null,
          accountLabel: null,
          quotaPolicy,
        }}
        onHide={vi.fn()}
        onRefresh={vi.fn()}
        onUpdatePolicy={onUpdatePolicy}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "打开设置" }));
    fireEvent.click(screen.getByRole("tab", { name: "额度" }));
    fireEvent.input(screen.getByLabelText("周一额度"), {
      target: { value: "20" },
    });
    expect(
      screen.getByRole("button", { name: "保存额度策略" }),
    ).toBeDisabled();
    expect(screen.getByRole("alert")).toHaveTextContent("不能超过 100%");

    fireEvent.input(screen.getByLabelText("周一额度"), {
      target: { value: "15" },
    });
    fireEvent.click(screen.getByRole("button", { name: "保存额度策略" }));

    await waitFor(() => {
      expect(onUpdatePolicy).toHaveBeenCalledWith(0, {
        policyTimezone: "Asia/Shanghai",
        carryWorkdaysEnabled: true,
        baseMicropoints: [
          15_000_000, 16_000_000, 16_000_000, 16_000_000, 16_000_000,
          10_000_000, 10_000_000,
        ],
      });
    });
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
    expect(
      screen.getByRole("button", { name: "正在刷新" }),
    ).toBeDisabled();
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

    expect(
      screen.getByRole("button", { name: "正在刷新" }),
    ).toBeDisabled();
    expect(screen.getByText("Codex 额度 · 正在刷新")).toBeInTheDocument();
    fireEvent.keyDown(window, { key: "r", ctrlKey: true });
    expect(onRefresh).not.toHaveBeenCalled();
  });
});
