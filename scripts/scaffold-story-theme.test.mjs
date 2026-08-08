import assert from "node:assert/strict";
import { mkdtemp, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import {
  scaffoldStoryTheme,
  validateStoryThemeId,
} from "./scaffold-story-theme.mjs";

test("story theme ids share the persisted settings contract", () => {
  assert.equal(validateStoryThemeId("signal_garden"), "signal_garden");
  assert.throws(() => validateStoryThemeId("Signal-Garden"));
  assert.throws(() => validateStoryThemeId("9_signal"));
});

test("scaffolds a local scene and preview without overwriting", async () => {
  const root = await mkdtemp(join(tmpdir(), "quotatide-story-theme-"));
  const result = await scaffoldStoryTheme({
    root,
    themeId: "signal_garden",
    titleZh: "信号花园",
    titleEn: "Signal Garden",
  });
  const scene = await readFile(join(result.target, "Scene.tsx"), "utf8");
  const preview = await readFile(join(result.target, "Preview.tsx"), "utf8");
  assert.match(scene, /data-story-theme="signal_garden"/u);
  assert.match(scene, /data-story-display=\{displayMode\}/u);
  assert.match(preview, /SignalGardenPreview/u);
  await assert.rejects(() => scaffoldStoryTheme({
    root,
    themeId: "signal_garden",
    titleZh: "信号花园",
    titleEn: "Signal Garden",
  }));
});
