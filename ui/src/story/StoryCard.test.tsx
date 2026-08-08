// @vitest-environment jsdom

import "@testing-library/jest-dom/vitest";
import { cleanup, render, screen } from "@testing-library/preact";
import { afterEach, describe, expect, it } from "vitest";

import { StoryCard, storyThemeOptions } from ".";
import type { StorySource } from "./model";

const source: StorySource = {
  weeklyUsed: "57%",
  weeklyRemaining: "43%",
  pressure: "warning",
  burnProjection: {
    rate: "1.2%/小时",
    projectedUsage: "68%",
    conclusion: "按当前速度，到重置预计使用 68%",
  },
  resetAbsolute: "2026-08-10 00:00",
  resetRelative: "4 天后",
  radar: null,
};

afterEach(cleanup);

describe("StoryCard interface", () => {
  it("renders every registered adapter from the same quota facts", () => {
    const { rerender } = render(
      <StoryCard themeId="rising_water" source={source} />,
    );

    expect(screen.getByRole("group", { name: /周额度压力舱/ }))
      .toHaveAttribute("data-story-theme", "rising_water");
    expect(screen.getByRole("group", { name: /已用 57%，剩余 43%/ }))
      .toBeInTheDocument();

    rerender(<StoryCard themeId="last_supply_line" source={source} />);

    expect(screen.getByRole("group", { name: /七日围城/ }))
      .toHaveAttribute("data-story-theme", "last_supply_line");
    expect(screen.getByRole("group", { name: /周补给剩余 43%/ }))
      .toBeInTheDocument();

    rerender(<StoryCard themeId="orbital_beacon" source={source} />);

    expect(screen.getByRole("group", { name: /轨道信标/ }))
      .toHaveAttribute("data-story-theme", "orbital_beacon");
    expect(screen.getByRole("group", { name: /周储备剩余 43%/ }))
      .toBeInTheDocument();
  });

  it("falls back to the default adapter for an unavailable theme id", () => {
    render(<StoryCard themeId="future_theme" source={source} />);

    expect(screen.getByRole("group", { name: /周额度压力舱/ }))
      .toHaveAttribute("data-story-theme", "rising_water");
  });

  it("provides localized settings options from the registry", () => {
    expect(storyThemeOptions("zh-CN")).toEqual([
      { id: "rising_water", title: "水位上涨" },
      { id: "last_supply_line", title: "七日围城" },
      { id: "orbital_beacon", title: "轨道信标" },
    ]);
    expect(storyThemeOptions("en")).toEqual([
      { id: "rising_water", title: "Rising Water" },
      { id: "last_supply_line", title: "Last Supply Line" },
      { id: "orbital_beacon", title: "Orbital Beacon" },
    ]);
  });

  it("applies the same shared display mode to every adapter", () => {
    const { rerender } = render(
      <StoryCard themeId="rising_water" source={source} displayMode="expanded" />,
    );

    expect(screen.getByRole("group", { name: /周额度压力舱/ }))
      .toHaveAttribute("data-story-display", "expanded");

    rerender(
      <StoryCard
        themeId="last_supply_line"
        source={source}
        displayMode="expanded"
      />,
    );
    expect(screen.getByRole("group", { name: /七日围城/ }))
      .toHaveAttribute("data-story-display", "expanded");

    rerender(
      <StoryCard themeId="orbital_beacon" source={source} displayMode="expanded" />,
    );
    expect(screen.getByRole("group", { name: /轨道信标/ }))
      .toHaveAttribute("data-story-display", "expanded");
  });
});
