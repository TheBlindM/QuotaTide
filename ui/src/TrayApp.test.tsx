// @vitest-environment jsdom

import "@testing-library/jest-dom/vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/preact";
import { afterEach, describe, expect, it, vi } from "vitest";

import { TrayApp } from "./TrayApp";
import { ledgerFixtures } from "./WeeklyLedger";

afterEach(cleanup);

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
    expect(screen.getByLabelText("auth.json 路径")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "返回额度" }));
    expect(
      screen.getByRole("table", { name: /当前七日窗口/ }),
    ).toBeInTheDocument();
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
});
