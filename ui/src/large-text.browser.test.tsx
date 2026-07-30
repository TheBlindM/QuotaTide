import { render } from "preact";
import { afterEach, expect, test } from "vitest";
import { page } from "vitest/browser";

import { App } from "./App";
import { I18nProvider } from "./i18n-context";
import "./styles.css";

let root: HTMLDivElement | null = null;

function requireElement(selector: string): HTMLElement {
  const element = document.querySelector<HTMLElement>(selector);
  expect(element, `Expected ${selector} to exist`).not.toBeNull();
  return element as HTMLElement;
}

function expectNoHorizontalOverflow(element: HTMLElement): void {
  expect(element.scrollWidth).toBe(element.clientWidth);
}

function revealIn(
  scrollContainer: HTMLElement,
  element: HTMLElement,
  focus = true,
): void {
  if (focus) {
    element.focus();
  }
  element.scrollIntoView({ block: "center", inline: "nearest" });
  const containerRect = scrollContainer.getBoundingClientRect();
  const elementRect = element.getBoundingClientRect();
  expect(elementRect.top).toBeGreaterThanOrEqual(containerRect.top - 1);
  expect(elementRect.bottom).toBeLessThanOrEqual(containerRect.bottom + 1);
  if (focus) {
    expect(document.activeElement).toBe(element);
  }
}

afterEach(() => {
  if (root !== null) {
    render(null, root);
    root.remove();
    root = null;
  }
  window.history.replaceState({}, "", window.location.pathname);
  delete document.documentElement.dataset.fontScale;
  delete document.documentElement.dataset.surface;
  delete document.documentElement.dataset.theme;
});

test("keeps every core English workflow reachable at 420×680 and 200% text", async () => {
  window.history.replaceState(
    {},
    "",
    `${window.location.pathname}?preview&state=warning&radar=active&lang=en&format=en-US&fontScale=2&surface=opaque`,
  );
  root = document.createElement("div");
  root.id = "root";
  document.body.append(root);
  render(
    <I18nProvider preference="system">
      <App />
    </I18nProvider>,
    root,
  );

  const ledger = requireElement(".weekly-ledger");
  const ledgerContent = requireElement(".ledger-content");
  expectNoHorizontalOverflow(ledger);
  expectNoHorizontalOverflow(ledgerContent);

  revealIn(
    ledger,
    requireElement('button[aria-label="Refresh now"]'),
  );
  revealIn(
    ledgerContent,
    requireElement("#window-heading"),
    false,
  );
  const radarLinks = document.querySelectorAll<HTMLAnchorElement>(
    ".radar-card a",
  );
  expect(radarLinks).toHaveLength(2);
  for (const link of radarLinks) {
    revealIn(ledgerContent, link);
  }

  await page.getByRole("button", { name: "Settings", exact: true }).click();
  await expect
    .element(page.getByRole("heading", { name: "Settings", exact: true }))
    .toBeVisible();

  const settings = requireElement(".settings-view");
  const header = requireElement(".settings-header");
  const subtitle = requireElement(".settings-header p");
  const content = requireElement(".settings-content");
  expectNoHorizontalOverflow(settings);
  expectNoHorizontalOverflow(header);
  expect(subtitle.scrollHeight).toBe(subtitle.clientHeight);
  expect(getComputedStyle(subtitle).whiteSpace).toBe("normal");
  expectNoHorizontalOverflow(content);
  expect(content.scrollHeight).toBeGreaterThan(content.clientHeight);

  revealIn(
    content,
    requireElement('input[aria-label="auth.json path"]'),
  );
  revealIn(
    settings,
    requireElement("button.settings-save"),
  );

  await page.getByRole("tab", { name: "Quota" }).click();
  expectNoHorizontalOverflow(content);
  revealIn(
    content,
    requireElement('input[aria-label="Mon quota"]'),
  );
  revealIn(
    content,
    requireElement(
      'input[aria-label="Dynamic workday carry"]',
    ),
  );
  revealIn(
    content,
    requireElement('input[aria-label="Policy timezone"]'),
  );

  await page.getByRole("tab", { name: "Alerts" }).click();
  expectNoHorizontalOverflow(content);
  revealIn(
    content,
    requireElement(".notification-status button"),
  );
  revealIn(
    content,
    requireElement(
      'input[aria-label="Daily quota reaches 80% system alert"]',
    ),
  );
  revealIn(
    content,
    requireElement(
      'input[aria-label="Enable email notifications"]',
    ),
  );

  await page.getByRole("tab", { name: "Privacy" }).click();
  await expect
    .element(page.getByRole("button", { name: "Save all settings" }))
    .toBeVisible();
  expectNoHorizontalOverflow(content);
  revealIn(
    content,
    requireElement(".privacy-tool button"),
  );
  const privacyButtons =
    document.querySelectorAll<HTMLButtonElement>(".privacy-tool button");
  expect(privacyButtons).toHaveLength(2);
  revealIn(content, privacyButtons[1]);

  const save = requireElement("button.settings-save");
  save.focus();
  const saveRect = save.getBoundingClientRect();
  const settingsRect = settings.getBoundingClientRect();
  expect(document.activeElement).toBe(save);
  expect(saveRect.top).toBeGreaterThanOrEqual(settingsRect.top);
  expect(saveRect.bottom).toBeLessThanOrEqual(settingsRect.bottom);
});
