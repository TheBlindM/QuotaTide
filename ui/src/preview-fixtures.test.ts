import { describe, expect, it } from "vitest";

import { createPreviewScenario } from "./preview-fixtures";

describe("quota preview mocks", () => {
  it.each([
    ["10", 10_000_000, "safe"],
    ["65", 65_000_000, "warning"],
    ["85", 85_000_000, "danger"],
    ["97", 97_000_000, "critical"],
  ] as const)(
    "maps %s%% used to the production pressure thresholds",
    (quota, usedMicropoints, pressure) => {
      const scenario = createPreviewScenario(
        new URLSearchParams({ quota }),
      );

      expect(scenario.liveQuota?.usedMicropoints).toBe(usedMicropoints);
      expect(scenario.liveQuota?.pressure).toBe(pressure);
    },
  );

  it("supports recovery and clamps unsafe mock percentages", () => {
    const recovery = createPreviewScenario(
      new URLSearchParams({ quota: "4", pressure: "recovery" }),
    );
    const clamped = createPreviewScenario(
      new URLSearchParams({ quota: "140" }),
    );

    expect(recovery.liveQuota?.pressure).toBe("recovery");
    expect(recovery.liveQuota?.burnProjection).toBeNull();
    expect(clamped.liveQuota?.usedMicropoints).toBe(100_000_000);
    expect(clamped.liveQuota?.remainingMicropoints).toBe(0);
  });
});
