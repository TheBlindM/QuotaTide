// @vitest-environment jsdom

import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/preact";
import { describe, expect, it, vi } from "vitest";

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

describe("QuotaTide skeleton", () => {
  it("shows public build information returned by the Rust command", async () => {
    render(<App />);

    expect(
      await screen.findByRole("heading", { name: "QuotaTide" }),
    ).toBeInTheDocument();
    expect(screen.getByText("Skeleton ready · 0.1.0")).toBeInTheDocument();
    expect(
      screen.getByText("Rust core and desktop shell are connected."),
    ).toBeInTheDocument();
  });
});
