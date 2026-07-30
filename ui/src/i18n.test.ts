import { describe, expect, it } from "vitest";

import {
  formatPercent,
  formatResetTime,
  messages,
  pluralizedMinutes,
  pseudoLocalize,
  resolveInterfaceLocale,
} from "./i18n";

describe("interface locale resolution", () => {
  it.each([
    ["zh", "zh-CN"],
    ["zh-CN", "zh-CN"],
    ["zh-SG", "zh-CN"],
    ["zh-Hans", "zh-CN"],
    ["zh-TW", "en"],
    ["zh-HK", "en"],
    ["zh-Hant", "en"],
    ["en", "en"],
    ["en-GB", "en"],
    ["fr-FR", "en"],
    ["not_a_locale", "en"],
    [null, "en"],
  ] as const)("maps system locale %s to %s", (source, expected) => {
    expect(resolveInterfaceLocale("system", source)).toBe(expected);
  });

  it("keeps an explicit language independent from the system locale", () => {
    expect(resolveInterfaceLocale("zh-CN", "en-US")).toBe("zh-CN");
    expect(resolveInterfaceLocale("en", "zh-CN")).toBe("en");
  });
});

describe("localized resources and formatters", () => {
  it("keeps the English and Simplified Chinese key sets identical", () => {
    expect(Object.keys(messages.en).sort()).toEqual(
      Object.keys(messages["zh-CN"]).sort(),
    );
  });

  it("formats percentages with at most one useful decimal", () => {
    expect(formatPercent(16_000_000, "en-US")).toBe("16%");
    expect(formatPercent(16_500_000, "en-US")).toBe("16.5%");
    expect(formatPercent(16_000_000, "zh-CN")).toBe("16%");
    expect(formatPercent(Number.NaN, "en-US")).toBe("");
  });

  it("uses minute precision and exposes relative and absolute reset time", () => {
    const reset = formatResetTime(
      Date.UTC(2026, 6, 30, 2, 1),
      Date.UTC(2026, 6, 30, 1, 58),
      "en",
      "en-US",
      "Asia/Shanghai",
    );

    expect(reset?.relative).toBe("in 3 minutes");
    expect(reset?.absolute).toContain("Jul 30, 2026");
    expect(reset?.accessible).toContain("reset time");
  });

  it("keeps English plural and Chinese minute messages deterministic", () => {
    expect(pluralizedMinutes("en", 1)).toBe("in 1 minute");
    expect(pluralizedMinutes("en", 2)).toBe("in 2 minutes");
    expect(pluralizedMinutes("zh-CN", -2)).toBe("2 分钟前");
  });

  it("expands pseudo-localized copy by at least forty percent", () => {
    const source = "Save all settings";
    const pseudo = pseudoLocalize(source);
    expect(pseudo).toMatch(/^⟦/u);
    expect(pseudo.length).toBeGreaterThanOrEqual(Math.ceil(source.length * 1.4));
  });
});
