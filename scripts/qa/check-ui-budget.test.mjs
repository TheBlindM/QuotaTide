import assert from "node:assert/strict";
import test from "node:test";

import {
  assertUiBudgets,
  isVisualAsset,
} from "./check-ui-budget.mjs";

test("UI budget keeps executable code separate from encoded visual assets", () => {
  assert.equal(isVisualAsset("assets/story.webp"), true);
  assert.equal(isVisualAsset("assets/background.PNG"), true);
  assert.equal(isVisualAsset("assets/index.js"), false);
  assert.equal(isVisualAsset("assets/pet.json"), false);

  assert.doesNotThrow(() =>
    assertUiBudgets({ codeTotal: 100, visualTotal: 200 }, 100, 200),
  );
  assert.throws(
    () => assertUiBudgets({ codeTotal: 101, visualTotal: 200 }, 100, 200),
    /UI code gzip budget exceeded/,
  );
  assert.throws(
    () => assertUiBudgets({ codeTotal: 100, visualTotal: 201 }, 100, 200),
    /UI visual asset gzip budget exceeded/,
  );
});
