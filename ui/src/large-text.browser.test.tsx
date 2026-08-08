import { render } from "preact";
import { afterEach, expect, test } from "vitest";
import { page } from "vitest/browser";

import { App } from "./App";
import { I18nProvider } from "./i18n-context";
import { ledgerFixtures, WeeklyLedger } from "./WeeklyLedger";
import "./styles.css";

let root: HTMLDivElement | null = null;

function requireElement(selector: string): HTMLElement {
  const element = document.querySelector<HTMLElement>(selector);
  expect(element, `Expected ${selector} to exist`).not.toBeNull();
  return element as HTMLElement;
}

function expectNoHorizontalOverflow(element: HTMLElement): void {
  const elementRect = element.getBoundingClientRect();
  const offenders = Array.from(element.querySelectorAll<HTMLElement>("*"))
    .filter((candidate) => {
      const rect = candidate.getBoundingClientRect();
      return (
        candidate.scrollWidth > candidate.clientWidth + 1 ||
        rect.left < elementRect.left - 1 ||
        rect.right > elementRect.right + 1
      );
    })
    .slice(0, 5)
    .map((candidate) => {
      const rect = candidate.getBoundingClientRect();
      return `${candidate.className || candidate.tagName} (${String(candidate.scrollWidth)}/${String(candidate.clientWidth)}; ${rect.left.toFixed(1)}–${rect.right.toFixed(1)})`;
    })
    .join(", ");
  expect(
    element.scrollWidth,
    `${element.className || element.tagName} should not overflow horizontally; offenders: ${offenders || "none"}`,
  ).toBe(element.clientWidth);
}

function revealIn(
  scrollContainer: HTMLElement,
  element: HTMLElement,
  focus = true,
): void {
  if (focus) {
    element.focus();
  }
  let containerRect = scrollContainer.getBoundingClientRect();
  let elementRect = element.getBoundingClientRect();
  if (
    elementRect.top < containerRect.top ||
    elementRect.bottom > containerRect.bottom
  ) {
    element.scrollIntoView({ block: "center", inline: "nearest" });
    containerRect = scrollContainer.getBoundingClientRect();
    elementRect = element.getBoundingClientRect();
  }
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
  delete document.documentElement.dataset.runtime;
  delete document.documentElement.dataset.surface;
  delete document.documentElement.dataset.theme;
  window.localStorage.removeItem("quotatide.theme");
});

test("uses desktop cursors for surfaces, help, actions, and text input", () => {
  root = document.createElement("div");
  root.id = "root";
  root.style.width = "360px";
  root.style.height = "430px";
  document.body.append(root);
  render(
    <I18nProvider preference="system">
      <WeeklyLedger
        fixture={{ ...ledgerFixtures.fresh, pressure: "danger" }}
        onOpenSettings={() => undefined}
        onRefresh={() => undefined}
      />
    </I18nProvider>,
    root,
  );

  expect(getComputedStyle(document.body).cursor).toBe("default");
  expect(getComputedStyle(requireElement(".weekly-ledger")).cursor).toBe(
    "default",
  );
  expect(
    getComputedStyle(requireElement(".quota-side-stat__state")).cursor,
  ).toBe("help");
  expect(getComputedStyle(requireElement("button")).cursor).toBe("pointer");

  const input = document.createElement("input");
  root.append(input);
  expect(getComputedStyle(input).cursor).toBe("text");
});

test("fits the expanded reset announcement in the compact shell with one divider", () => {
  const announcement = ledgerFixtures.fresh.radar?.announcement;
  expect(announcement).toBeDefined();
  root = document.createElement("div");
  root.id = "root";
  root.style.width = "360px";
  root.style.height = "430px";
  document.body.append(root);
  render(
    <I18nProvider preference="system">
      <WeeklyLedger
        fixture={{
          ...ledgerFixtures.fresh,
          radar: {
            kind: "empty",
            message: "当前无计划重置信号",
            announcement: announcement ?? null,
          },
        }}
        onOpenSettings={() => undefined}
        onRefresh={() => undefined}
      />
    </I18nProvider>,
    root,
  );

  const summary = requireElement(".command-summary");
  const window = requireElement(".ledger-window");
  const footer = requireElement(".ledger-footer");
  expect(getComputedStyle(summary).borderBottomWidth).toBe("0px");
  expect(getComputedStyle(window).borderTopWidth).toBe("1px");
  expect(
    footer.getBoundingClientRect().top - window.getBoundingClientRect().bottom,
  ).toBeLessThanOrEqual(16);
});

test("matches the selected C telemetry layout at 360×460 in both themes", async () => {
  window.history.replaceState(
    {},
    "",
    `${window.location.pathname}?preview&state=fresh&radar=active&theme=light`,
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
  const content = requireElement(".ledger-content");
  const ledgerRect = ledger.getBoundingClientRect();
  expect(ledgerRect.width).toBe(360);
  expect(ledgerRect.height).toBe(460);
  await new Promise((resolve) => window.setTimeout(resolve, 50));
  expect(document.documentElement.dataset.runtime).toBe("preview");
  const previewShadow = getComputedStyle(ledger).boxShadow;
  expect(previewShadow).not.toBe("none");
  document.documentElement.dataset.runtime = "desktop";
  const desktopShadow = getComputedStyle(ledger).boxShadow;
  expect(desktopShadow).toContain("inset");
  expect(desktopShadow).not.toContain("58px");
  expect(desktopShadow).not.toBe(previewShadow);
  document.documentElement.dataset.runtime = "preview";
  expect(getComputedStyle(ledger).backgroundSize).not.toContain("16px");
  for (const selector of [
    ".side-stat span",
    ".side-stat small",
    ".ledger-window__heading span",
    ".ledger-window__toggle",
    ".ledger-day > span",
    ".ledger-day > small",
    ".radar-card__header span",
    ".radar-card small",
    ".ledger-footer button",
    ".ledger-footer > span",
  ]) {
    const elements = document.querySelectorAll<HTMLElement>(selector);
    expect(elements.length, `Expected ${selector} to exist`).toBeGreaterThan(0);
    for (const element of elements) {
      expect(
        Number.parseFloat(getComputedStyle(element).fontSize),
        `${selector} should remain legible in the compact shell`,
      ).toBeGreaterThanOrEqual(9);
    }
  }
  expectNoHorizontalOverflow(ledger);
  expectNoHorizontalOverflow(content);
  const chamberViewport = requireElement(".quota-chamber__viewport");
  const chamber = requireElement(".quota-chamber");
  const chamberWidth = chamber.getBoundingClientRect().width;
  expect(
    chamberWidth,
    "Every story theme should use the shared compact summary slot",
  ).toBeGreaterThan(150);
  expect(chamberWidth).toBeLessThan(220);
  const valve = requireElement(".quota-chamber__valve");
  const robot = requireElement(".quota-robot");
  expect(document.querySelector(".quota-robot__tether")).toBeNull();
  const valveRect = valve.getBoundingClientRect();
  const robotRect = robot.getBoundingClientRect();
  expect(robotRect.width).toBe(48);
  expect(robotRect.height).toBe(52);
  const sprite = requireElement(".quota-robot__sprite");
  expect(getComputedStyle(sprite).backgroundSize).toBe("384px 468px");
  expect(getComputedStyle(sprite).animationDuration).toContain("1.2s");
  expect(
    valveRect.bottom,
    "The reset valve must remain visually separate from the robot",
  ).toBeLessThanOrEqual(robotRect.top + 1);
  const resetChip = requireElement(".quota-chamber__reset-chip");
  const viewportRect = chamberViewport.getBoundingClientRect();
  const resetChipRect = resetChip.getBoundingClientRect();
  expect(resetChipRect.right).toBeLessThanOrEqual(viewportRect.right);
  expect(resetChipRect.bottom).toBeLessThanOrEqual(viewportRect.bottom);
  expect(document.querySelector(".quota-chamber__projection")).toBeNull();
  expect(chamberViewport.querySelector(".quota-chamber__forecast")?.children)
    .toHaveLength(0);
  document.documentElement.dataset.runtime = "desktop";
  expect(
    content.scrollHeight,
    "The compact desktop overview should fit in one screen without a vertical scroll range",
  ).toBe(content.clientHeight);
  content.scrollTop = 100;
  expect(content.scrollTop).toBe(0);
  document.documentElement.dataset.runtime = "preview";

  for (const selector of [
    ".command-summary",
    ".ledger-window",
    ".radar-card",
  ]) {
    const element = requireElement(selector);
    const rect = element.getBoundingClientRect();
    const contentRect = content.getBoundingClientRect();
    expect(rect.top).toBeGreaterThanOrEqual(contentRect.top - 1);
    expect(rect.bottom).toBeLessThanOrEqual(contentRect.bottom + 1);
  }

  await page
    .getByRole("list", { name: "本周策略 07/24 至 07/30" })
    .getByText("今天", { exact: true })
    .hover();
  await new Promise((resolve) => window.setTimeout(resolve, 250));
  const tooltip = requireElement("#ledger-day-inspector");
  const tooltipRect = tooltip.getBoundingClientRect();
  expect(getComputedStyle(tooltip).opacity).toBe("1");
  expect(tooltip.textContent).toContain("11.4%");
  expect(tooltip.textContent).toContain("16.8%");
  expect(tooltip.textContent).toContain("5.4%");
  expect(tooltipRect.left).toBeGreaterThanOrEqual(ledgerRect.left);
  expect(tooltipRect.right).toBeLessThanOrEqual(ledgerRect.right);

  const lightBackground = getComputedStyle(ledger).backgroundColor;
  await page.getByRole("button", { name: "切换到夜间模式" }).click();
  await new Promise((resolve) => window.setTimeout(resolve, 220));
  expect(document.documentElement.dataset.theme).toBe("dark");
  expect(window.localStorage.getItem("quotatide.theme")).toBe("dark");
  expect(getComputedStyle(ledger).backgroundColor).not.toBe(lightBackground);
  await expect
    .element(page.getByRole("button", { name: "切换到日间模式" }))
    .toBeVisible();
});

test("keeps settings navigation fixed and scrolls inside the active card", async () => {
  window.history.replaceState(
    {},
    "",
    `${window.location.pathname}?preview&theme=dark`,
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

  await page.getByRole("button", { name: "设置" }).click();
  const content = requireElement(".settings-content");
  const tabs = requireElement(".settings-tabs");
  const panel = requireElement("#settings-panel-account");
  const tabsTop = tabs.getBoundingClientRect().top;

  expect(content.scrollHeight).toBe(content.clientHeight);
  content.scrollTop = 240;
  expect(content.scrollTop).toBe(0);
  expect(panel.scrollHeight).toBeGreaterThan(panel.clientHeight);
  panel.scrollTop = 240;
  await new Promise((resolve) => window.setTimeout(resolve, 60));
  expect(panel.scrollTop).toBeGreaterThan(0);
  expect(tabs.getBoundingClientRect().top).toBe(tabsTop);
  const contentRect = content.getBoundingClientRect();
  const panelRect = panel.getBoundingClientRect();
  expect(panelRect.top).toBeGreaterThan(contentRect.top);
  expect(panelRect.bottom).toBeLessThanOrEqual(contentRect.bottom);

  const themePicker = requireElement(".story-theme-picker");
  const themeRadios = themePicker.querySelectorAll<HTMLInputElement>(
    'input[type="radio"]',
  );
  expect(themeRadios).toHaveLength(3);
  expectNoHorizontalOverflow(themePicker);
  const siegeRadio = Array.from(themeRadios).find(
    (radio) => radio.value === "last_supply_line",
  );
  expect(siegeRadio).toBeDefined();
  siegeRadio?.click();
  expect(siegeRadio?.checked).toBe(true);
  expect(requireElement(".last-supply-line-preview").getBoundingClientRect().height)
    .toBe(52);
});

test("renders generated artwork for the Last Supply Line scene", () => {
  window.history.replaceState(
    {},
    "",
    `${window.location.pathname}?preview&state=warning&radar=active&story=last_supply_line&theme=dark`,
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

  expect(getComputedStyle(requireElement(".supply-line__scene")).backgroundImage)
    .toContain("/assets/siege-v2/background");
  expect(getComputedStyle(requireElement(".siege-zombie")).backgroundImage)
    .toContain("/assets/siege-v2/zombie-actions");
  expect(getComputedStyle(requireElement(".siege-defender")).backgroundImage)
    .toContain("/assets/siege-v2/survivor-actions");
  expect(getComputedStyle(requireElement(".supply-line__barricade")).backgroundImage)
    .toContain("/assets/siege-v2/supply-props");
  const supplyLine = requireElement(".supply-line");
  const supplyLineWidth = supplyLine.getBoundingClientRect().width;
  expect(
    supplyLineWidth,
    "Every story theme should use the shared compact summary slot",
  ).toBeGreaterThan(150);
  expect(supplyLineWidth).toBeLessThan(220);
  expect(supplyLine.style.getPropertyValue("--supply-level")).toBe("55%");
  expect(getComputedStyle(requireElement(".supply-line__signal-dot")).animationName)
    .toContain("siege-signal-dot");
  const supplyLineRect = supplyLine.getBoundingClientRect();
  supplyLine.dispatchEvent(new PointerEvent("pointermove", {
    bubbles: true,
    clientX: supplyLineRect.right - 2,
    clientY: supplyLineRect.top + 2,
  }));
  expect(Number.parseFloat(supplyLine.style.getPropertyValue("--story-pointer-x")))
    .toBeGreaterThan(0.9);
  const closestZombieEdge = Math.max(
    ...Array.from(document.querySelectorAll<HTMLElement>(".siege-zombie"))
      .map((zombie) => zombie.getBoundingClientRect().right),
  );
  const frontDefender = requireElement(".siege-defender--front")
    .getBoundingClientRect();
  expect(
    frontDefender.left - closestZombieEdge,
    "The warning state should preserve a readable no-man's-land between both sides",
  ).toBeGreaterThanOrEqual(20);
});

test("expands every story adapter through the shared display contract", async () => {
  window.history.replaceState(
    {},
    "",
    `${window.location.pathname}?preview&state=warning&radar=active&story=orbital_beacon&theme=dark`,
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

  const scene = requireElement(".orbital-beacon");
  expect(scene.getBoundingClientRect().width).toBeLessThan(220);
  expect(scene).toHaveAttribute("data-story-display", "compact");

  await page.getByRole("button", { name: "展开故事场景" }).click();
  expect(scene.getBoundingClientRect().width).toBeGreaterThan(300);
  expect(scene).toHaveAttribute("data-story-display", "expanded");
  expect(
    getComputedStyle(requireElement(".side-stats")).gridTemplateColumns.split(" "),
  ).toHaveLength(2);
  expectNoHorizontalOverflow(requireElement(".command-summary"));

  await page.getByRole("button", { name: "收起故事场景" }).click();
  expect(scene.getBoundingClientRect().width).toBeLessThan(220);
  expect(scene).toHaveAttribute("data-story-display", "compact");
});

test("plays the RPG clear-and-reset sequence when supplies arrive", () => {
  window.history.replaceState(
    {},
    "",
    `${window.location.pathname}?preview&quota=93&pressure=recovery&story=last_supply_line&theme=dark`,
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

  const rpg = requireElement(".siege-defender--rpg");
  const rocket = requireElement(".siege-rocket");
  const blast = requireElement(".siege-blast");
  const horde = requireElement(".supply-line__horde");
  const airdrop = requireElement(".supply-line__airdrop");
  expect(getComputedStyle(rpg).backgroundImage)
    .toContain("/assets/siege-v2/survivor-rpg-actions");
  expect(getComputedStyle(rocket).backgroundImage)
    .toContain("/assets/siege-v2/rpg-effects");
  expect(getComputedStyle(blast).backgroundImage)
    .toContain("/assets/siege-v2/rpg-effects");
  expect(getComputedStyle(rpg).animationName).toContain("survivor-rpg-clear");
  expect(getComputedStyle(rocket).animationName).toContain("rpg-rocket-flight");
  expect(getComputedStyle(blast).animationName).toContain("rpg-impact-burst");
  expect(getComputedStyle(horde).animationName).toContain("siege-horde-clear");
  expect(getComputedStyle(horde).animationDuration).toBe("6.4s");
  expect(getComputedStyle(airdrop).backgroundImage)
    .toContain("/assets/siege-v2/supply-props");
  expect(getComputedStyle(airdrop).animationName)
    .toContain("supply-airdrop-arrival");
});

test("drives water level, pressure color, and pet sprite from mocked quota", async () => {
  const cases = [
    { quota: 10, pressure: "safe" },
    { quota: 65, pressure: "warning" },
    { quota: 85, pressure: "danger" },
    { quota: 97, pressure: "critical" },
  ] as const;
  const renderedWaterHeights: number[] = [];
  const spriteRows = new Set<string>();
  let safeWaterBackground = "";
  let criticalWaterBackground = "";

  for (const mock of cases) {
    window.history.replaceState(
      {},
      "",
      `${window.location.pathname}?preview&quota=${String(mock.quota)}&theme=dark`,
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
    await new Promise((resolve) => window.setTimeout(resolve, 50));

    const chamber = requireElement(".quota-chamber");
    const viewport = requireElement(".quota-chamber__viewport");
    const water = requireElement(".quota-water");
    const wave = requireElement(".quota-water__wave");
    const waveLine = wave.querySelector(".quota-water__line") as SVGPathElement;
    const robot = requireElement(".quota-robot");
    const sprite = requireElement(".quota-robot__sprite");
    const viewportHeight = viewport.getBoundingClientRect().height;
    const viewportTop = viewport.getBoundingClientRect().top;
    const waterTop = water.getBoundingClientRect().top;
    const waterHeight = water.getBoundingClientRect().height;

    expect(chamber).toHaveClass(`pressure-${mock.pressure}`);
    expect(chamber.getAttribute("style")).toContain(
      `--water-level: ${String(mock.quota)}%`,
    );
    expect(robot).toHaveClass(`quota-robot--${mock.pressure}`);
    expect(wave.querySelectorAll("path")).toHaveLength(2);
    expect(waveLine).not.toBeNull();
    if (mock.pressure === "warning") {
      const initialWavePath = waveLine.getAttribute("d");
      const waveChanged = await new Promise<boolean>((resolve) => {
        let animationFrameId = 0;
        const timeoutId = window.setTimeout(() => {
          window.cancelAnimationFrame(animationFrameId);
          resolve(false);
        }, 750);
        const inspectFrame = () => {
          if (waveLine.getAttribute("d") !== initialWavePath) {
            window.clearTimeout(timeoutId);
            resolve(true);
            return;
          }
          animationFrameId = window.requestAnimationFrame(inspectFrame);
        };
        animationFrameId = window.requestAnimationFrame(inspectFrame);
      });
      expect(waveChanged).toBe(true);
    }
    expect(waterHeight).toBeGreaterThanOrEqual(
      viewportHeight * (mock.quota / 100) * 0.76 - 1,
    );
    expect(waterHeight).toBeLessThanOrEqual(
      viewportHeight * (mock.quota / 100) * 0.76 + 1,
    );
    if (mock.pressure === "critical") {
      expect(waterTop).toBeGreaterThanOrEqual(
        viewportTop + viewportHeight * 0.24 - 1,
      );
    }

    renderedWaterHeights.push(waterHeight);
    spriteRows.add(getComputedStyle(sprite).backgroundPositionY);
    if (mock.pressure === "safe") {
      safeWaterBackground = getComputedStyle(water, "::after").backgroundImage;
    }
    if (mock.pressure === "critical") {
      criticalWaterBackground = getComputedStyle(water, "::after").backgroundImage;
    }

    render(null, root);
    root.remove();
    root = null;
  }

  expect(renderedWaterHeights).toEqual(
    [...renderedWaterHeights].sort((left, right) => left - right),
  );
  expect(spriteRows.size).toBe(cases.length);
  expect(criticalWaterBackground).not.toBe(safeWaterBackground);

  window.history.replaceState(
    {},
    "",
    `${window.location.pathname}?preview&quota=4&pressure=recovery&theme=dark`,
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
  await new Promise((resolve) => window.setTimeout(resolve, 50));
  expect(requireElement(".quota-chamber")).toHaveClass("pressure-recovery");
  expect(requireElement(".quota-robot")).toHaveClass("quota-robot--recovery");
  expect(getComputedStyle(requireElement(".quota-water")).animationName).toBe(
    "chamber-drain",
  );
});

test("keeps the warning overview stationary and day quota details above the footer", async () => {
  window.history.replaceState(
    {},
    "",
    `${window.location.pathname}?preview&state=warning&radar=active&theme=dark`,
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

  const content = requireElement(".ledger-content");
  const footer = requireElement(".ledger-footer");
  expect(content.scrollHeight).toBe(content.clientHeight);
  content.scrollTop = 100;
  expect(content.scrollTop).toBe(0);

  await page
    .getByRole("list", { name: "本周策略 07/24 至 07/30" })
    .getByText("今天", { exact: true })
    .hover();
  await new Promise((resolve) => window.setTimeout(resolve, 250));

  const quotaDetail = requireElement("#ledger-day-inspector");
  const detailRect = quotaDetail.getBoundingClientRect();
  const focusedDay = requireElement(".ledger-day.is-inspected");
  const otherDay = requireElement(
    ".ledger-week > [role='listitem']:not(.is-inspected) .ledger-day",
  );
  const weekRect = requireElement(".ledger-week").getBoundingClientRect();
  const footerRect = footer.getBoundingClientRect();
  expect(focusedDay.getBoundingClientRect().width).toBeGreaterThan(
    otherDay.getBoundingClientRect().width * 3,
  );
  expect(detailRect.left).toBeGreaterThanOrEqual(weekRect.left);
  expect(detailRect.right).toBeLessThanOrEqual(weekRect.right);
  expect(detailRect.bottom).toBeLessThanOrEqual(footerRect.top);
});

test("morphs the compact rail into a complete vertical week", async () => {
  window.history.replaceState(
    {},
    "",
    `${window.location.pathname}?preview&state=fresh&radar=active&theme=dark`,
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

  const switcher = requireElement(".ledger-week-switcher");
  const collapsedHeight = switcher.getBoundingClientRect().height;
  expect(collapsedHeight).toBeLessThan(70);
  expect(getComputedStyle(switcher).transitionDuration).not.toBe("0s");

  await page.getByRole("button", { name: "查看明细" }).click();
  await new Promise((resolve) => window.setTimeout(resolve, 500));

  const detail = requireElement("#ledger-week-detail");
  expect(detail.getAttribute("aria-hidden")).toBe("false");
  expect(detail.querySelectorAll('[role="listitem"]')).toHaveLength(7);
  expect(switcher.getBoundingClientRect().height).toBeGreaterThan(240);
  expectNoHorizontalOverflow(detail);
  await expect
    .element(page.getByRole("button", { name: "收起明细" }))
    .toBeVisible();
});

test("keeps every core English workflow reachable at 360×460 and 200% text", async () => {
  window.history.replaceState(
    {},
    "",
    `${window.location.pathname}?preview&state=warning&radar=active&story=last_supply_line&lang=en&format=en-US&fontScale=2&surface=opaque`,
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
  await expect.element(page.getByRole("group", { name: /Last Supply Line/ })).toBeVisible();
  expectNoHorizontalOverflow(ledger);
  expectNoHorizontalOverflow(ledgerContent);
  const compactSupplyLine = requireElement(".supply-line").getBoundingClientRect();
  const compactLedger = ledger.getBoundingClientRect();
  expect(compactSupplyLine.left).toBeGreaterThanOrEqual(compactLedger.left);
  expect(compactSupplyLine.right).toBeLessThanOrEqual(compactLedger.right);

  revealIn(
    ledger,
    requireElement('button[aria-label="Refresh now"]'),
  );
  revealIn(
    ledgerContent,
    requireElement("#window-heading"),
    false,
  );
  await page
    .getByText("Predicted reset · Third-party signal", { exact: true })
    .click();
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
  expect(getComputedStyle(subtitle).display).toBe("none");
  expectNoHorizontalOverflow(content);
  expect(getComputedStyle(content).overflowY).toBe("hidden");
  expect(
    requireElement("#settings-panel-account").scrollHeight,
  ).toBeGreaterThan(requireElement("#settings-panel-account").clientHeight);

  revealIn(
    requireElement("#settings-panel-account"),
    requireElement('input[aria-label="auth.json path"]'),
  );
  await page
    .getByRole("textbox", { name: "auth.json path" })
    .fill("/Users/me/.codex/auth.json");
  revealIn(
    settings,
    requireElement("button.settings-save"),
  );

  await page.getByRole("tab", { name: "Quota" }).click();
  expectNoHorizontalOverflow(content);
  const quotaPanel = requireElement("#settings-panel-quota");
  revealIn(
    quotaPanel,
    requireElement('input[aria-label="Mon quota"]'),
  );
  revealIn(
    quotaPanel,
    requireElement(
      'input[aria-label="Dynamic workday carry"]',
    ),
  );
  revealIn(
    quotaPanel,
    requireElement('input[aria-label="Policy timezone"]'),
  );

  await page.getByRole("tab", { name: "Alerts" }).click();
  expectNoHorizontalOverflow(content);
  const alertsPanel = requireElement("#settings-panel-alerts");
  revealIn(
    alertsPanel,
    requireElement(".notification-status button"),
  );
  revealIn(
    alertsPanel,
    requireElement(
      'input[aria-label="Daily quota reaches 80% system alert"]',
    ),
  );
  revealIn(
    alertsPanel,
    requireElement(
      'input[aria-label="Enable email notifications"]',
    ),
  );

  await page.getByRole("tab", { name: "Privacy" }).click();
  await expect
    .element(page.getByRole("button", { name: "Save all settings" }))
    .toBeVisible();
  expectNoHorizontalOverflow(content);
  const privacyPanel = requireElement("#settings-panel-privacy");
  revealIn(
    privacyPanel,
    requireElement(".privacy-tool button"),
  );
  const privacyButtons =
    document.querySelectorAll<HTMLButtonElement>(".privacy-tool button");
  expect(privacyButtons).toHaveLength(2);
  revealIn(privacyPanel, privacyButtons[1]);

  const save = requireElement("button.settings-save");
  save.focus();
  const saveRect = save.getBoundingClientRect();
  const settingsRect = settings.getBoundingClientRect();
  expect(document.activeElement).toBe(save);
  expect(saveRect.top).toBeGreaterThanOrEqual(settingsRect.top);
  expect(saveRect.bottom).toBeLessThanOrEqual(settingsRect.bottom);
});
