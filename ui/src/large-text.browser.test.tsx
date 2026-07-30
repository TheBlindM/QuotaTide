import { render } from "preact";
import { afterEach, expect, test } from "vitest";
import { page } from "vitest/browser";

import { App } from "./App";
import { I18nProvider } from "./i18n-context";
import "./styles.css";

let root: HTMLDivElement | null = null;

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

test("keeps the English settings workflow reachable at 420×680 and 200% text", async () => {
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

  await page.getByRole("button", { name: "Settings", exact: true }).click();
  await expect
    .element(page.getByRole("heading", { name: "Settings", exact: true }))
    .toBeVisible();

  const settings = document.querySelector<HTMLElement>(".settings-view");
  const header = document.querySelector<HTMLElement>(".settings-header");
  const subtitle = document.querySelector<HTMLElement>(".settings-header p");
  const content = document.querySelector<HTMLElement>(".settings-content");
  expect(settings).not.toBeNull();
  expect(header).not.toBeNull();
  expect(subtitle).not.toBeNull();
  expect(content).not.toBeNull();
  expect(settings?.scrollWidth).toBe(settings?.clientWidth);
  expect(header?.scrollWidth).toBe(header?.clientWidth);
  expect(subtitle?.scrollHeight).toBe(subtitle?.clientHeight);
  expect(getComputedStyle(subtitle as HTMLElement).whiteSpace).toBe("normal");
  expect(content?.scrollWidth).toBe(content?.clientWidth);
  expect(content?.scrollHeight).toBeGreaterThan(content?.clientHeight ?? 0);

  await page.getByRole("tab", { name: "Privacy" }).click();
  await expect
    .element(page.getByRole("button", { name: "Save all settings" }))
    .toBeVisible();
  const privacyContent = document.querySelector<HTMLElement>(".settings-content");
  expect(privacyContent?.scrollWidth).toBe(privacyContent?.clientWidth);
  privacyContent?.scrollTo({ top: privacyContent.scrollHeight });
  await expect.element(page.getByRole("button", { name: "Delete…" })).toBeVisible();
});
