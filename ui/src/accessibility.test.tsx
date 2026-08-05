// @vitest-environment jsdom

import "@testing-library/jest-dom/vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/preact";
import axe from "axe-core";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { PublicAlertInbox } from "./bindings/PublicAlertInbox";
import { TrayApp } from "./TrayApp";
import { ledgerFixtures } from "./WeeklyLedger";

afterEach(cleanup);

async function expectNoSeriousAccessibilityViolations(): Promise<void> {
  const result = await axe.run(document.body, {
    rules: {
      // jsdom does not provide layout or computed pixel colors. Contrast is
      // covered by the forced-colors tokens and manual platform release pass.
      "color-contrast": { enabled: false },
    },
  });
  expect(
    result.violations.filter(
      (violation) =>
        violation.impact === "critical" || violation.impact === "serious",
    ),
  ).toEqual([]);
}

describe("automated accessibility gate", () => {
  it("has no critical or serious violations in the weekly overview", async () => {
    render(
      <TrayApp
        fixture={ledgerFixtures.fresh}
        onHide={vi.fn()}
        onRefresh={vi.fn()}
      />,
    );
    await expectNoSeriousAccessibilityViolations();
  });

  it("has no critical or serious violations in the top message popover", async () => {
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
      ],
    };
    render(
      <TrayApp
        fixture={ledgerFixtures.fresh}
        alerts={alerts}
        onDismissAlert={vi.fn()}
        onDismissAllAlerts={vi.fn()}
        onHide={vi.fn()}
        onRefresh={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "打开消息" }));
    await expectNoSeriousAccessibilityViolations();
  });

  it("has no critical or serious violations across every settings panel", async () => {
    render(
      <TrayApp
        fixture={ledgerFixtures.fresh}
        onHide={vi.fn()}
        onRefresh={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "设置" }));
    for (const tabName of ["账号", "额度", "提醒", "隐私"]) {
      fireEvent.click(screen.getByRole("tab", { name: tabName }));
      await expectNoSeriousAccessibilityViolations();
    }
  });
});
