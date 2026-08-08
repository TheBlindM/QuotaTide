import type { StoryTheme } from "../bindings/StoryTheme";
import type { InterfaceLocale } from "../i18n";
import { useI18n } from "../i18n-context";
import { createStorySnapshot, type StorySource } from "./model";
import { LastSupplyLinePreview } from "./themes/last-supply-line/Preview";
import { LastSupplyLineScene } from "./themes/last-supply-line/Scene";
import { OrbitalBeaconPreview } from "./themes/orbital-beacon/Preview";
import { OrbitalBeaconScene } from "./themes/orbital-beacon/Scene";
import { RisingWaterPreview } from "./themes/rising-water/Preview";
import { RisingWaterScene } from "./themes/rising-water/Scene";
import type { StoryThemeAdapter } from "./types";
import "./layout.css";
import "./picker.css";

const DEFAULT_STORY_THEME: StoryTheme = "rising_water";

const storyThemeAdapters: readonly StoryThemeAdapter[] = [
  {
    id: "rising_water",
    title: (locale) => locale === "zh-CN" ? "水位上涨" : "Rising Water",
    description: (locale) =>
      locale === "zh-CN" ? "压力舱·流体水位" : "Pressure chamber · fluid level",
    Preview: RisingWaterPreview,
    Scene: RisingWaterScene,
  },
  {
    id: "last_supply_line",
    title: (locale) => locale === "zh-CN" ? "七日围城" : "Last Supply Line",
    description: (locale) =>
      locale === "zh-CN" ? "末日防线·补给信号" : "Siege line · supply signal",
    Preview: LastSupplyLinePreview,
    Scene: LastSupplyLineScene,
  },
  {
    id: "orbital_beacon",
    title: (locale) => locale === "zh-CN" ? "轨道信标" : "Orbital Beacon",
    description: (locale) =>
      locale === "zh-CN" ? "深空雷达·储备轨道" : "Deep-space radar · reserve orbit",
    Preview: OrbitalBeaconPreview,
    Scene: OrbitalBeaconScene,
  },
];

const storyThemeRegistry = new Map(
  storyThemeAdapters.map((adapter) => [adapter.id, adapter] as const),
);

function resolveStoryTheme(themeId: string): StoryThemeAdapter {
  return storyThemeRegistry.get(themeId) ??
    storyThemeRegistry.get(DEFAULT_STORY_THEME) as StoryThemeAdapter;
}

export function storyThemeOptions(locale: InterfaceLocale): ReadonlyArray<{
  id: StoryTheme;
  title: string;
}> {
  return storyThemeAdapters.map((adapter) => ({
    id: adapter.id,
    title: adapter.title(locale),
  }));
}

export function StoryCard({
  themeId,
  source,
  displayMode = "compact",
}: {
  themeId: string;
  source: StorySource;
  displayMode?: "compact" | "expanded";
}) {
  const adapter = resolveStoryTheme(themeId);
  return (
    <adapter.Scene
      key={adapter.id}
      snapshot={createStorySnapshot(source)}
      displayMode={displayMode}
    />
  );
}

export function StoryThemePicker({
  value,
  onChange,
}: {
  value: string;
  onChange: (themeId: StoryTheme) => void;
}) {
  const { locale, text } = useI18n();
  const selectedAdapter = resolveStoryTheme(value);

  return (
    <div
      class="story-theme-picker"
      role="radiogroup"
      aria-label={text("故事主题", "Story theme")}
    >
      {storyThemeAdapters.map((adapter) => {
        const selected = adapter.id === selectedAdapter.id;
        return (
          <label
            key={adapter.id}
            class={`story-theme-option${selected ? " is-selected" : ""}`}
          >
            <input
              type="radio"
              name="story-theme"
              value={adapter.id}
              checked={selected}
              onChange={() => {
                onChange(adapter.id);
              }}
            />
            <adapter.Preview />
            <span class="story-theme-option__copy">
              <strong>{adapter.title(locale)}</strong>
              <small>{adapter.description(locale)}</small>
            </span>
            <span class="story-theme-option__check" aria-hidden="true">
              {selected ? "✓" : ""}
            </span>
          </label>
        );
      })}
    </div>
  );
}
