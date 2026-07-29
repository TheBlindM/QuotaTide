// @vitest-environment jsdom

import "@testing-library/jest-dom/vitest";
import { cleanup, render, screen, within } from "@testing-library/preact";
import { afterEach, describe, expect, it, vi } from "vitest";

import { WeeklyLedger, ledgerFixtures } from "./WeeklyLedger";

afterEach(cleanup);

describe("Weekly Ledger overview", () => {
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
    expect(screen.getByText(/连续 3 次刷新失败/)).toBeInTheDocument();
    expect(
      screen.getByRole("table", { name: /当前七日窗口/ }),
    ).toBeInTheDocument();

    screen.getByRole("button", { name: "重试" }).click();
    expect(onRefresh).toHaveBeenCalledOnce();
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
});
