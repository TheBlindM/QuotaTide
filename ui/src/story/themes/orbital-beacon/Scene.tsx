import { useI18n } from "../../../i18n-context";
import { pressureLabel } from "../../model";
import type { StorySceneProps } from "../../types";
import "./theme.css";

export function OrbitalBeaconScene({ snapshot, displayMode }: StorySceneProps) {
  const { locale, text } = useI18n();
  const state = pressureLabel(snapshot.pressure, locale);
  const signal = snapshot.radarChance ?? text("未锁定", "Unresolved");

  return (
    <div
      class={`primary-stat orbital-beacon pressure-${snapshot.pressure}`}
      data-story-theme="orbital_beacon"
      data-story-display={displayMode}
      role="group"
      aria-label={text(
        `轨道信标：周储备剩余 ${snapshot.weeklyRemainingLabel}，${state}。下次窗口 ${snapshot.resetRelative}，雷达信号 ${signal}。`,
        `Orbital Beacon: ${snapshot.weeklyRemainingLabel} weekly reserve remains, ${state}. Next window ${snapshot.resetRelative}; radar signal ${signal}.`,
      )}
      style={`--orbital-reserve:${String(snapshot.weeklyRemaining)}%;--orbital-used:${String(snapshot.weeklyUsed)}%`}
    >
      <div class="orbital-beacon__scene" aria-hidden="true">
        <span class="orbital-beacon__grid" />
        <span class="orbital-beacon__orbit orbital-beacon__orbit--outer" />
        <span class="orbital-beacon__orbit orbital-beacon__orbit--middle" />
        <span class="orbital-beacon__orbit orbital-beacon__orbit--inner" />
        <span class="orbital-beacon__sweep" />
        <span class="orbital-beacon__core">
          <i />
        </span>
        <span class="orbital-beacon__blip orbital-beacon__blip--one" />
        <span class="orbital-beacon__blip orbital-beacon__blip--two" />
        <span class="orbital-beacon__horizon" />
      </div>
      <div class="orbital-beacon__readout">
        <span>{text("轨道储备", "ORBITAL RESERVE")}</span>
        <strong>{snapshot.weeklyRemainingLabel}</strong>
        <small>{state}</small>
      </div>
      <div class="orbital-beacon__telemetry">
        <span>{text("下次窗口", "NEXT WINDOW")}</span>
        <strong>{snapshot.resetRelative}</strong>
        <i aria-hidden="true" />
        <small>{text("信标", "BEACON")} {signal}</small>
      </div>
    </div>
  );
}
