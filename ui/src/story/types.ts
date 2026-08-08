import type { ComponentType } from "preact";

import type { StoryTheme } from "../bindings/StoryTheme";
import type { InterfaceLocale } from "../i18n";
import type { StorySnapshot } from "./model";

export type StorySceneProps = Readonly<{
  snapshot: StorySnapshot;
  displayMode: "compact" | "expanded";
  motionActive: boolean;
}>;

export type StoryThemeAdapter = Readonly<{
  id: StoryTheme;
  title: (locale: InterfaceLocale) => string;
  description: (locale: InterfaceLocale) => string;
  Preview: ComponentType;
  Scene: ComponentType<StorySceneProps>;
}>;
