// @vitest-environment jsdom

import "@testing-library/jest-dom/vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/preact";
import { act } from "preact/test-utils";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { PublicAlertInbox } from "./bindings/PublicAlertInbox";
import { WeeklyLedger, ledgerFixtures } from "./WeeklyLedger";

afterEach(cleanup);

describe("Weekly Ledger overview", () => {
  it("renders the selected last-supply-line story without changing quota facts", () => {
    render(
      <WeeklyLedger
        fixture={ledgerFixtures.over}
        storyTheme="last_supply_line"
        onOpenSettings={vi.fn()}
        onRefresh={vi.fn()}
      />,
    );

    const scene = screen.getByRole("group", { name: /七日围城/ });
    expect(scene).toHaveTextContent("周补给");
    expect(scene).toHaveTextContent(ledgerFixtures.over.weeklyRemaining);
    expect(scene).toHaveTextContent("防线承压");
    expect(scene.querySelectorAll(".siege-zombie")).toHaveLength(8);
    expect(screen.queryByRole("group", { name: /周额度压力舱/ })).not.toBeInTheDocument();
  });

  it("turns the supply signal into an arrival when reset recovery is confirmed", () => {
    render(
      <WeeklyLedger
        fixture={{ ...ledgerFixtures.fresh, pressure: "recovery" }}
        storyTheme="last_supply_line"
        onOpenSettings={vi.fn()}
        onRefresh={vi.fn()}
      />,
    );

    expect(screen.getByText("已抵达")).toBeInTheDocument();
    expect(screen.getByTestId("supply-airdrop")).toBeInTheDocument();
    expect(screen.queryByTestId("supply-convoy")).not.toBeInTheDocument();
    expect(screen.getByTestId("siege-rpg")).toBeInTheDocument();
    expect(screen.getByTestId("siege-rocket")).toBeInTheDocument();
    expect(screen.getByTestId("siege-blast")).toBeInTheDocument();
  });

  it("moves the horde continuously with weekly usage instead of pressure bands", () => {
    const { rerender } = render(
      <WeeklyLedger
        fixture={{
          ...ledgerFixtures.fresh,
          pressure: "warning",
          weeklyRemaining: "90%",
        }}
        storyTheme="last_supply_line"
        onOpenSettings={vi.fn()}
        onRefresh={vi.fn()}
      />,
    );

    const scene = screen.getByRole("group", { name: /七日围城/ });
    const earlyAdvance = Number.parseFloat(
      scene.style.getPropertyValue("--siege-advance"),
    );

    rerender(
      <WeeklyLedger
        fixture={{
          ...ledgerFixtures.fresh,
          pressure: "warning",
          weeklyRemaining: "40%",
        }}
        storyTheme="last_supply_line"
        onOpenSettings={vi.fn()}
        onRefresh={vi.fn()}
      />,
    );

    const laterAdvance = Number.parseFloat(
      scene.style.getPropertyValue("--siege-advance"),
    );
    expect(earlyAdvance).toBeCloseTo(9);
    expect(laterAdvance).toBeCloseTo(19);
    expect(laterAdvance).toBeGreaterThan(earlyAdvance);
  });

  it("refocuses the same notification target for every activation", async () => {
    const { rerender } = render(
      <WeeklyLedger
        fixture={ledgerFixtures.fresh}
        focusTarget="today"
        focusActivationId={1}
        onOpenSettings={vi.fn()}
        onRefresh={vi.fn()}
      />,
    );

    await waitFor(() => {
      expect(document.activeElement).toHaveAttribute(
        "id",
        "quota-target-today",
      );
    });
    (document.activeElement as HTMLElement).blur();

    rerender(
      <WeeklyLedger
        fixture={ledgerFixtures.fresh}
        focusTarget="today"
        focusActivationId={2}
        onOpenSettings={vi.fn()}
        onRefresh={vi.fn()}
      />,
    );

    await waitFor(() => {
      expect(document.activeElement).toHaveAttribute(
        "id",
        "quota-target-today",
      );
    });
  });

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
        focusActivationId={1}
        onOpenSettings={vi.fn()}
        onRefresh={vi.fn()}
      />,
    );

    expect(
      screen.queryByRole("region", { name: "最近提醒" }),
    ).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "打开消息" }));
    expect(screen.getByRole("region", { name: "最近提醒" })).toHaveTextContent(
      "今日额度已达到 80%",
    );
    expect(screen.getByText(/系统通知未授权/)).toBeInTheDocument();
    expect(document.activeElement).toHaveAttribute("id", "quota-target-today");
  });

  it("keeps a safe in-app error visible when system notification delivery fails", () => {
    const alerts: PublicAlertInbox = {
      notificationPermissionStatus: "granted",
      events: [
        {
          eventId: 42,
          eventKind: "weekly_remaining_10",
          localDate: null,
          source: null,
          target: "today",
          systemDeliveryState: "retry_wait",
          createdAtUnixMs: 1_785_347_200_000,
        },
      ],
    };

    render(
      <WeeklyLedger
        fixture={ledgerFixtures.fresh}
        alerts={alerts}
        onOpenSettings={vi.fn()}
        onRefresh={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "打开消息" }));
    expect(screen.getByRole("region", { name: "最近提醒" })).toHaveTextContent(
      "本周额度仅剩 10%",
    );
    expect(screen.getByText(/系统通知发送失败/)).toHaveTextContent(
      "应用内提醒已保留",
    );
  });

  it("keeps one settings entry and supports deleting or clearing top-bar messages", () => {
    const alerts: PublicAlertInbox = {
      notificationPermissionStatus: "granted",
      events: [
        {
          eventId: 41,
          eventKind: "daily_80",
          localDate: "2026-07-30",
          source: null,
          target: "today",
          systemDeliveryState: "delivered",
          createdAtUnixMs: 1_785_347_200_000,
        },
        {
          eventId: 42,
          eventKind: "weekly_remaining_10",
          localDate: null,
          source: null,
          target: "today",
          systemDeliveryState: "delivered",
          createdAtUnixMs: 1_785_347_201_000,
        },
      ],
    };
    const dismissAlert = vi.fn();
    const dismissAllAlerts = vi.fn();

    render(
      <WeeklyLedger
        fixture={ledgerFixtures.fresh}
        alerts={alerts}
        onDismissAlert={dismissAlert}
        onDismissAllAlerts={dismissAllAlerts}
        onOpenSettings={vi.fn()}
        onRefresh={vi.fn()}
      />,
    );

    expect(
      screen.queryByRole("button", { name: "打开设置" }),
    ).not.toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: "设置" })).toHaveLength(1);

    const messageButton = screen.getByRole("button", { name: "打开消息" });
    expect(messageButton).toHaveTextContent("2");
    fireEvent.click(messageButton);
    fireEvent.click(
      screen.getByRole("button", {
        name: "删除提醒：今日额度已达到 80%",
      }),
    );
    expect(dismissAlert).toHaveBeenCalledWith(41);

    fireEvent.click(screen.getByRole("button", { name: "清空全部" }));
    expect(dismissAllAlerts).toHaveBeenCalledOnce();
  });

  it("shows the current account's compact seven-day rail and expands the complete week", async () => {
    const onWeekDetailChange = vi.fn();
    render(
      <WeeklyLedger
        fixture={ledgerFixtures.fresh}
        onWeekDetailChange={onWeekDetailChange}
        onOpenSettings={vi.fn()}
        onRefresh={vi.fn()}
      />,
    );

    expect(screen.getByRole("heading", { name: "QuotaTide" })).toBeInTheDocument();
    expect(screen.getByText("周剩余")).toBeInTheDocument();
    expect(screen.getByText("58%")).toBeInTheDocument();
    expect(screen.getByText("Codex 额度 · 正常")).toBeInTheDocument();
    expect(screen.getByText("预计重置 · 第三方信号")).toBeInTheDocument();
    expect(screen.getByText(">70%")).toBeInTheDocument();
    expect(screen.getByText("置信度")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "查看原始来源" })).toHaveAttribute(
      "href",
      "https://x.com/thsottiaux/status/2081899343091843463",
    );

    const ledger = screen.getByRole("list", {
      name: "本周策略 07/24 至 07/30",
    });
    expect(within(ledger).getAllByRole("listitem")).toHaveLength(7);
    const todaySummary = within(ledger).getByRole("listitem", {
      name: "今天 07/28 · 正常",
    });
    const todayDay = todaySummary.querySelector<HTMLElement>(".ledger-day");
    expect(todayDay).not.toBeNull();
    fireEvent.mouseEnter(todayDay as HTMLElement);
    const dayInspector = screen.getByRole("tooltip", {
      name: "今天 额度明细",
    });
    expect(dayInspector).toHaveTextContent("正常");
    expect(dayInspector).toHaveTextContent("已用11.4%");
    expect(dayInspector).toHaveTextContent("上限16.8%");
    expect(dayInspector).toHaveTextContent("可用5.4%");
    expect(
      screen.queryByRole("region", { name: "整周额度明细" }),
    ).not.toBeInTheDocument();

    screen.getByRole("button", { name: "查看明细" }).click();
    expect(onWeekDetailChange).toHaveBeenCalledWith(true);
    await waitFor(() => {
      const detail = screen.getByRole("region", { name: "整周额度明细" });
      expect(within(detail).getAllByRole("listitem")).toHaveLength(7);
      expect(detail).toHaveTextContent("今天");
      expect(detail).toHaveTextContent("11.4%");
      expect(detail).toHaveTextContent("16.8%");
      expect(detail).toHaveTextContent("5.4%");
      expect(detail).toHaveTextContent("尚无记录");
    });
    expect(
      screen.queryByRole("region", { name: "今天 额度明细" }),
    ).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "收起明细" })).toHaveAttribute(
      "aria-expanded",
      "true",
    );
    screen.getByRole("button", { name: "收起明细" }).click();
    expect(onWeekDetailChange).toHaveBeenLastCalledWith(false);
  });

  it("labels a mid-window baseline as quota suggested from now", () => {
    const suggestedFixture = {
      ...ledgerFixtures.fresh,
      todayAvailable: "17.3%",
      todayAvailabilityKind: "suggested_from_now" as const,
      days: ledgerFixtures.fresh.days.map((day, index) => ({
        ...day,
        used: index < 4 ? day.used : null,
        suggested:
          index < 4
            ? null
            : [17.333334, 17.333333, 17.333333][index - 4],
        status: index < 4 ? day.status : "尚无记录",
      })),
    };
    render(
      <WeeklyLedger
        fixture={suggestedFixture}
        onOpenSettings={vi.fn()}
        onRefresh={vi.fn()}
      />,
    );

    expect(screen.getByText("从现在建议可用")).toBeInTheDocument();
    const todaySummary = screen.getByRole("listitem", {
      name: "今天 07/28 · 尚无记录",
    });
    const todayDay = todaySummary.querySelector<HTMLElement>(".ledger-day");
    expect(todayDay).not.toBeNull();
    fireEvent.mouseEnter(todayDay as HTMLElement);
    const inspector = screen.getByRole("tooltip", { name: "今天 额度明细" });
    expect(inspector).toHaveTextContent("本机已记录0.0%");
    expect(inspector).toHaveTextContent("计划上限17.3%");
    expect(inspector).toHaveTextContent("建议可用17.3%");

    fireEvent.click(screen.getByRole("button", { name: "查看明细" }));
    const futureDay = screen.getByRole("listitem", {
      name: "周三 07/29 · 尚无记录",
    });
    expect(futureDay).toHaveTextContent("17.3%");
  });

  it("renders the selected C telemetry hierarchy instead of the generic card dashboard", () => {
    render(
      <WeeklyLedger
        fixture={ledgerFixtures.fresh}
        onOpenSettings={vi.fn()}
        onRefresh={vi.fn()}
      />,
    );

    expect(screen.getByText("Q")).toHaveClass("ledger-mark");
    expect(screen.getByText(/CODEX · LIVE/u)).toHaveClass("ledger-live-line");
    const console = screen.getByRole("region", { name: "额度控制台" });
    expect(console).toHaveClass("command-summary");
    expect(within(console).getByText("周剩余")).toBeInTheDocument();
    expect(within(console).getByText(ledgerFixtures.fresh.resetRelative))
      .toHaveAttribute("title", ledgerFixtures.fresh.resetAbsolute);
    expect(
      within(console).getByRole("region", { name: "重置雷达" }),
    ).toHaveClass("radar-card--summary");
    expect(
      screen.queryByText("重置时间", { selector: ".side-stat span" }),
    ).not.toBeInTheDocument();
  });

  it("turns weekly usage and exhaustion pressure into one rising-water chamber", () => {
    const pressuredFixture = {
      ...ledgerFixtures.fresh,
      pressure: "critical" as const,
      weeklyUsed: "76%",
      weeklyRemaining: "24%",
      burnProjection: {
        rate: "1.0%/小时",
        projectedUsage: "142%",
        conclusion: "预计周六 14:00 触顶，早于重置",
      },
    };

    render(
      <WeeklyLedger
        fixture={pressuredFixture}
        onOpenSettings={vi.fn()}
        onRefresh={vi.fn()}
      />,
    );

    const chamber = screen.getByRole("group", {
      name: /周额度压力舱：已用 76%，剩余 24%，临界/u,
    });
    expect(chamber).toHaveClass("pressure-critical");
    expect(chamber.getAttribute("style")).toContain("--water-level: 76%");
    expect(chamber.getAttribute("style")).toContain("--water-height: 57.76%");
    expect(chamber).not.toHaveTextContent("RESET LOCKED");
    expect(chamber.querySelector(".quota-chamber__forecast"))
      .toBeEmptyDOMElement();
    expect(chamber.querySelectorAll(".quota-water__wave")).toHaveLength(1);
    expect(chamber.querySelector(".quota-water__wave--back")).toBeNull();
    expect(chamber.querySelector(".quota-water i")).toBeNull();
    expect(chamber.querySelector(".quota-chamber__projection")).toBeNull();
    expect(chamber.querySelector(".quota-chamber__reset-chip"))
      .toHaveTextContent(pressuredFixture.resetRelative);
    expect(chamber).toHaveAccessibleName(/预计周六 14:00 触顶，早于重置/u);
    expect(chamber).toHaveAccessibleName(/重置/u);
    expect(screen.queryByText("1.0%/小时")).not.toBeInTheDocument();
  });

  it("explains when high pressure comes from projected usage", () => {
    render(
      <WeeklyLedger
        fixture={{
          ...ledgerFixtures.fresh,
          pressure: "danger",
          weeklyUsed: "63%",
          weeklyRemaining: "37%",
          burnProjection: {
            rate: "0.8%/小时",
            projectedUsage: "83.1%",
            conclusion: "按当前速度，到重置预计使用 83.1%",
          },
        }}
        onOpenSettings={vi.fn()}
        onRefresh={vi.fn()}
      />,
    );

    expect(screen.getByText("高压")).toHaveAttribute(
      "title",
      "当前消耗过快，预测到重置时会用到 83.1%（≥ 80%）。",
    );
  });

  it("explains when high pressure comes from current usage", () => {
    render(
      <WeeklyLedger
        fixture={{
          ...ledgerFixtures.fresh,
          pressure: "danger",
          weeklyUsed: "86%",
          weeklyRemaining: "14%",
          burnProjection: {
            rate: "0.3%/小时",
            projectedUsage: "92%",
            conclusion: "按当前速度，到重置预计使用 92%",
          },
        }}
        onOpenSettings={vi.fn()}
        onRefresh={vi.fn()}
      />,
    );

    expect(screen.getByText("高压")).toHaveAttribute(
      "title",
      "周额度已用 86%（≥ 80%），额度快用完了。",
    );
  });

  it("cycles through multiple pet actions inside one pressure state", async () => {
    vi.useFakeTimers();
    const rendered = render(
      <WeeklyLedger
        fixture={ledgerFixtures.fresh}
        onOpenSettings={vi.fn()}
        onRefresh={vi.fn()}
      />,
    );

    try {
      expect(document.querySelector(".quota-robot")).toHaveAttribute(
        "data-action",
        "idle",
      );
      await act(async () => {
        await vi.advanceTimersByTimeAsync(3_600);
      });
      expect(document.querySelector(".quota-robot")).toHaveAttribute(
        "data-action",
        "waving",
      );
      await act(async () => {
        await vi.advanceTimersByTimeAsync(2_160);
      });
      expect(document.querySelector(".quota-robot")).toHaveAttribute(
        "data-action",
        "idle",
      );
      await act(async () => {
        await vi.advanceTimersByTimeAsync(3_600);
      });
      expect(document.querySelector(".quota-robot")).toHaveAttribute(
        "data-action",
        "jumping",
      );
    } finally {
      rendered.unmount();
      vi.useRealTimers();
    }
  });

  it("keeps earned reset credits read-only and inside expanded details", async () => {
    const fixture = {
      ...ledgerFixtures.fresh,
      resetCredits: {
        availableLabel: "可用 2 次",
        expiryLabel: "最近一枚 3 天后到期",
      },
    };
    render(
      <WeeklyLedger
        fixture={fixture}
        onOpenSettings={vi.fn()}
        onRefresh={vi.fn()}
      />,
    );

    expect(
      screen.queryByRole("region", { name: "整周额度明细" }),
    ).not.toBeInTheDocument();
    screen.getByRole("button", { name: "查看明细" }).click();
    await waitFor(() => {
      expect(screen.getByText("重置券")).toBeInTheDocument();
    });
    expect(screen.getByText("可用 2 次")).toBeInTheDocument();
    expect(screen.getByText("最近一枚 3 天后到期")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /使用重置券/u }))
      .not.toBeInTheDocument();
  });

  it("moves daily warnings into the available-today card and usage rail", async () => {
    render(
      <WeeklyLedger
        fixture={ledgerFixtures.warning}
        onOpenSettings={vi.fn()}
        onRefresh={vi.fn()}
      />,
    );

    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(screen.getByLabelText(/今天还可用 2\.6%/u)).toHaveClass(
      "side-stat--warning",
    );
    expect(
      screen
        .getByRole("listitem", { name: /今天 07\/28 · 预警/u })
        .querySelector(".ledger-day"),
    ).toHaveClass("usage-warning");

    screen.getByRole("button", { name: "查看明细" }).click();
    await waitFor(() => {
      expect(
        screen.getByRole("region", { name: "整周额度明细" }),
      ).toHaveTextContent("14.2%");
    });
    expect(
      within(screen.getByRole("region", { name: "整周额度明细" }))
        .getAllByRole("listitem"),
    ).toHaveLength(7);
  });

  it("moves an exceeded daily limit into a dangerous available-today card", () => {
    render(
      <WeeklyLedger
        fixture={ledgerFixtures.over}
        onOpenSettings={vi.fn()}
        onRefresh={vi.fn()}
      />,
    );

    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(screen.getByLabelText(/今天还可用 0%；用量状态：已超额/u)).toHaveClass(
      "side-stat--danger",
    );
    expect(
      screen.getByRole("listitem", { name: /今天 07\/28 · 超额/ }),
    ).toBeInTheDocument();
    expect(
      screen
        .getByRole("listitem", { name: /今天 07\/28 · 超额/u })
        .querySelector(".ledger-day"),
    ).toHaveClass("usage-danger");
    expect(screen.getByRole("list", { name: /本周策略/ })).toBeInTheDocument();
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
    expect(screen.getByRole("list", { name: /本周策略/ })).toBeInTheDocument();

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
    expect(screen.getByRole("list", { name: /本周策略/ })).toBeInTheDocument();
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
    expect(screen.queryByRole("list")).not.toBeInTheDocument();
    expect(screen.queryByText("预计重置 · 第三方信号")).not.toBeInTheDocument();
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
            message: "当前无计划重置信号",
            announcement: null,
          },
        }}
        onOpenSettings={vi.fn()}
        onRefresh={vi.fn()}
      />,
    );
    expect(screen.getByText("当前无计划重置信号")).toBeInTheDocument();
    expect(screen.getByText("重置动态")).toBeInTheDocument();
    cleanup();

    render(
      <WeeklyLedger
        fixture={{
          ...ledgerFixtures.fresh,
          radar: {
            kind: "empty",
            message: "重置数据暂不可用",
            announcement: null,
          },
        }}
        onOpenSettings={vi.fn()}
        onRefresh={vi.fn()}
      />,
    );
    expect(screen.getByText("重置数据暂不可用")).toBeInTheDocument();
  });

  it("shows the latest reset announcement directly without a disclosure step", () => {
    const announcement = ledgerFixtures.fresh.radar?.announcement;
    expect(announcement).toBeDefined();
    render(
      <WeeklyLedger
        fixture={{
          ...ledgerFixtures.fresh,
          radar: {
            kind: "empty",
            message: "当前无计划重置信号",
            announcement: announcement ?? null,
          },
        }}
        onOpenSettings={vi.fn()}
        onRefresh={vi.fn()}
      />,
    );

    expect(screen.queryByText("查看最近重置公告")).not.toBeInTheDocument();
    expect(screen.getByText("最近重置公告")).toBeInTheDocument();
    expect(
      screen.getByText("ChatGPT Work 与 Codex 用户的用量限制已重置。"),
    ).toBeVisible();
  });
});
