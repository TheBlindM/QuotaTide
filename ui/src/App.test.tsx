// @vitest-environment jsdom

import "@testing-library/jest-dom/vitest";
import { cleanup, render, screen } from "@testing-library/preact";
import { afterEach, describe, expect, it, vi } from "vitest";

import { App } from "./App";

vi.mock("./api/build-info", () => ({
  loadBuildInfo: vi.fn().mockResolvedValue({
    productName: "QuotaTide",
    version: "0.1.0",
    author: "TheBlind",
    identifier: "dev.theblind.quotatide",
    stage: "skeleton",
  }),
}));

afterEach(() => {
  cleanup();
  window.history.replaceState({}, "", "/");
  delete document.documentElement.dataset.theme;
  delete document.documentElement.dataset.surface;
});

describe("QuotaTide tray app", () => {
  it("waits for the Rust shell and then shows the weekly ledger", async () => {
    render(<App />);

    expect(
      await screen.findByRole("heading", { name: "QuotaTide" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("table", { name: "当前七日窗口 07/24 至 07/30" }),
    ).toBeInTheDocument();
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
});
