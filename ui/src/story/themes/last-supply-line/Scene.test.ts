import { describe, expect, it } from "vitest";

import { createSiegeBattleState } from "./Scene";

describe("createSiegeBattleState", () => {
  it("moves the horde continuously closer as weekly supply falls", () => {
    const plentiful = createSiegeBattleState(82, "safe");
    const halfway = createSiegeBattleState(50, "safe");
    const depleted = createSiegeBattleState(4, "safe");

    expect(plentiful.advance).toBeLessThan(halfway.advance);
    expect(halfway.advance).toBeLessThan(depleted.advance);
    expect(depleted.advance).toBeLessThanOrEqual(78);
  });

  it("changes battle intensity without snapping the horde position", () => {
    const warning = createSiegeBattleState(80, "warning");
    const danger = createSiegeBattleState(80, "danger");
    const critical = createSiegeBattleState(80, "critical");

    expect(warning.advance).toBe(danger.advance);
    expect(danger.advance).toBe(critical.advance);
    expect(warning.phase).toBe("approaching");
    expect(danger.phase).toBe("assault");
    expect(critical.phase).toBe("breach");
  });

  it("pulls the horde back when supplies arrive", () => {
    expect(createSiegeBattleState(3, "recovery")).toEqual({
      advance: 8,
      phase: "relief",
      threat: 97,
    });
  });
});
