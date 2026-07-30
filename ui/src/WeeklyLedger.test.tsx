// @vitest-environment jsdom

import "@testing-library/jest-dom/vitest";
import { cleanup, render, screen, within } from "@testing-library/preact";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { PublicAlertInbox } from "./bindings/PublicAlertInbox";
import { WeeklyLedger, ledgerFixtures } from "./WeeklyLedger";

afterEach(cleanup);

describe("Weekly Ledger overview", () => {
  it("keeps durable in-app reminders visible when system notification permission is denied", () => {
    const alerts: PublicAlertInbox = {
      notificationPermissionStatus: "denied",
      events: [
        {
          eventId: 41,
          eventKind: "daily_80",
          localDate: "2026-07-30",
          source: null,
          target: "today",
          systemDeliveryState: "paused_permission",
          createdAtUnixMs: 1_785_347_200_000,
        },
      ],
    };

    render(
      <WeeklyLedger
        fixture={ledgerFixtures.fresh}
        alerts={alerts}
        focusTarget="today"
        onOpenSettings={vi.fn()}
        onRefresh={vi.fn()}
      />,
    );

    expect(screen.getByRole("region", { name: "最近提醒" })).toHaveTextContent(
      "今日额度已达到 80%",
    );
    expect(screen.getByText(/系统通知未授权/)).toBeInTheDocument();
    expect(document.activeElement).toHaveAttribute("id", "quota-target-today");
  });

  it("shows the current account's complete seven-day window", () => {
    render(
      <WeeklyLedger
        fixture={ledgerFixtures.fresh}
        onOpenSettings={vi.fn()}
        onRefresh={vi.fn()}
      />,
    );

    expect(screen.getByRole("heading", { name: "QuotaTide" })).toBeInTheDocument();
    expect(screen.getByText("周剩余")).toBeInTheDocument();
    expect(screen.getByText("58%")).toBeInTheDocument();
    expect(screen.getByText("Codex 额度 · 正常")).toBeInTheDocument();
    expect(screen.getByText("重置雷达 · 第三方预测")).toBeInTheDocument();
    expect(screen.getByText(">70%")).toBeInTheDocument();
    expect(screen.getByText("第三方 AI 估算 · 非 OpenAI 承诺")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "查看原始来源" })).toHaveAttribute(
      "href",
      "https://x.com/thsottiaux/status/2081899343091843463",
    );

    const ledger = screen.getByRole("table", {
      name: "当前七日窗口 07/24 至 07/30",
    });
    expect(within(ledger).getAllByRole("row")).toHaveLength(8);
  });

  it("shows an actionable warning without hiding the ledger", () => {
    render(
      <WeeklyLedger
        fixture={ledgerFixtures.warning}
        onOpenSettings={vi.fn()}
        onRefresh={vi.fn()}
      />,
    );

    expect(screen.getByRole("alert")).toHaveTextContent("接近今日额度");
    expect(screen.getByRole("button", { name: "查看今日" })).toBeInTheDocument();
    expect(
      screen.getByRole("row", { name: /今天.*14.2% 已用.*预警/ }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("table", { name: /当前七日窗口/ }),
    ).toBeInTheDocument();
  });

  it("labels an exceeded daily limit with text as well as color", () => {
    render(
      <WeeklyLedger
        fixture={ledgerFixtures.over}
        onOpenSettings={vi.fn()}
        onRefresh={vi.fn()}
      />,
    );

    expect(screen.getByRole("alert")).toHaveTextContent("今日额度已超出");
    expect(screen.getAllByText("超额")).not.toHaveLength(0);
    expect(
      screen.getByRole("table", { name: /当前七日窗口/ }),
    ).toBeInTheDocument();
  });

  it("retains the last complete snapshot when the source is stale", () => {
    const onRefresh = vi.fn();
    render(
      <WeeklyLedger
        fixture={ledgerFixtures.stale}
        onOpenSettings={vi.fn()}
        onRefresh={onRefresh}
      />,
    );

    expect(screen.getByRole("alert")).toHaveTextContent("数据已过期");
    expect(screen.getByText(/连续 3 次失败/)).toBeInTheDocument();
    expect(
      screen.getByRole("table", { name: /当前七日窗口/ }),
    ).toBeInTheDocument();

    screen.getByRole("button", { name: "重试" }).click();
    expect(onRefresh).toHaveBeenCalledOnce();
  });

  it("announces the active source refresh without removing the snapshot", () => {
    render(
      <WeeklyLedger
        fixture={ledgerFixtures.fresh}
        onOpenSettings={vi.fn()}
        onRefresh={vi.fn()}
        refreshing
      />,
    );

    expect(screen.getByText("Codex 额度 · 正在刷新")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "正在刷新" })).toBeDisabled();
    expect(
      screen.getByRole("table", { name: /当前七日窗口/ }),
    ).toBeInTheDocument();
  });

  it("offers auth.json setup without showing invented quota data", () => {
    const onOpenSettings = vi.fn();
    render(
      <WeeklyLedger
        fixture={ledgerFixtures.unconfigured}
        onOpenSettings={onOpenSettings}
        onRefresh={vi.fn()}
      />,
    );

    expect(screen.getByRole("heading", { name: "连接 Codex 账号" })).toBeInTheDocument();
    expect(screen.getByText(/仅在本机读取/)).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    expect(screen.queryByText("重置雷达 · 第三方预测")).not.toBeInTheDocument();
    expect(screen.queryByText(/上次成功/)).not.toBeInTheDocument();

    screen.getByRole("button", { name: "选择 auth.json" }).click();
    expect(onOpenSettings).toHaveBeenCalledOnce();
  });

  it("distinguishes no current prediction from a Radar source failure", () => {
    render(
      <WeeklyLedger
        fixture={{
          ...ledgerFixtures.fresh,
          radar: {
            kind: "empty",
            message: "当前无有效预测",
            announcement: null,
          },
        }}
        onOpenSettings={vi.fn()}
        onRefresh={vi.fn()}
      />,
    );
    expect(screen.getByText("当前无有效预测")).toBeInTheDocument();
    cleanup();

    render(
      <WeeklyLedger
        fixture={{
          ...ledgerFixtures.fresh,
          radar: {
            kind: "empty",
            message: "预测数据暂不可用",
            announcement: null,
          },
        }}
        onOpenSettings={vi.fn()}
        onRefresh={vi.fn()}
      />,
    );
    expect(screen.getByText("预测数据暂不可用")).toBeInTheDocument();
  });
});
