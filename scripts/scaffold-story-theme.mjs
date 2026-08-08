import assert from "node:assert/strict";
import { access, mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const STORY_THEME_ID = /^[a-z][a-z0-9_]{0,47}$/u;

export function validateStoryThemeId(themeId) {
  assert.match(
    themeId,
    STORY_THEME_ID,
    "Theme id must match [a-z][a-z0-9_]{0,47}",
  );
  return themeId;
}

function pascalCase(themeId) {
  return themeId
    .split("_")
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join("");
}

async function pathExists(path) {
  try {
    await access(path);
    return true;
  } catch {
    return false;
  }
}

export async function scaffoldStoryTheme({
  root,
  themeId,
  titleZh,
  titleEn,
}) {
  validateStoryThemeId(themeId);
  assert.ok(titleZh.trim(), "Chinese title is required");
  assert.ok(titleEn.trim(), "English title is required");
  const directoryName = themeId.replaceAll("_", "-");
  const className = directoryName;
  const functionName = pascalCase(themeId);
  const target = resolve(root, "ui/src/story/themes", directoryName);
  assert.equal(await pathExists(target), false, `Theme already exists: ${target}`);
  await mkdir(target, { recursive: true });

  const files = {
    "Scene.tsx": `import { useI18n } from "../../../i18n-context";
import { pressureLabel } from "../../model";
import type { StorySceneProps } from "../../types";
import "./theme.css";

export function ${functionName}Scene({ snapshot, displayMode }: StorySceneProps) {
  const { locale, text } = useI18n();
  const state = pressureLabel(snapshot.pressure, locale);
  return (
    <div
      class={\`primary-stat ${className} pressure-\${snapshot.pressure}\`}
      data-story-theme="${themeId}"
      data-story-display={displayMode}
      role="group"
      aria-label={text(
        \`${titleZh}：周剩余 \${snapshot.weeklyRemainingLabel}，\${state}。\`,
        \`${titleEn}: \${snapshot.weeklyRemainingLabel} weekly quota remains, \${state}.\`,
      )}
    >
      <div class="${className}__scene" aria-hidden="true" />
      <strong>{snapshot.weeklyRemainingLabel}</strong>
      <small>{state}</small>
    </div>
  );
}
`,
    "Preview.tsx": `import "./preview.css";

export function ${functionName}Preview() {
  return (
    <span class="story-theme-preview ${className}-preview" aria-hidden="true" />
  );
}
`,
    "theme.css": `.command-summary > .${className}[data-story-theme="${themeId}"] {
  position: relative;
  min-height: 136px;
  overflow: hidden;
  border: 1px solid color-mix(in srgb, var(--accent) 34%, transparent);
  border-radius: 16px;
  background: var(--telemetry-primary);
}

.${className}__scene {
  position: absolute;
  inset: 0;
}
`,
    "preview.css": `.${className}-preview {
  background: var(--telemetry-primary);
}
`,
  };

  await Promise.all(
    Object.entries(files).map(([filename, contents]) =>
      writeFile(resolve(target, filename), contents, { flag: "wx" })
    ),
  );
  return { target, functionName, directoryName };
}

const invokedPath = process.argv[1] ? resolve(process.argv[1]) : null;
if (invokedPath === fileURLToPath(import.meta.url)) {
  const [themeId, titleZh, titleEn] = process.argv.slice(2);
  assert.ok(
    themeId && titleZh && titleEn,
    "Usage: npm run new:story-theme -- <theme_id> <Chinese title> <English title>",
  );
  const result = await scaffoldStoryTheme({
    root: resolve(import.meta.dirname, ".."),
    themeId,
    titleZh,
    titleEn,
  });
  console.log(`Created ${result.target}`);
  console.log(
    `Register ${result.functionName}Scene and ${result.functionName}Preview in ui/src/story/index.tsx`,
  );
}
