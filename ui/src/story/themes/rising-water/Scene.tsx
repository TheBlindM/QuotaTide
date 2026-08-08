import { useEffect, useRef, useState } from "preact/hooks";

import { useI18n } from "../../../i18n-context";
import { pressureLabel } from "../../model";
import type { StorySceneProps } from "../../types";
import "./theme.css";

type TideAction =
  | "idle"
  | "running-right"
  | "running-left"
  | "waving"
  | "jumping"
  | "failed"
  | "waiting"
  | "running"
  | "review";

const CHAMBER_WATER_CAP_RATIO = 0.76;
const CHAMBER_WAVE_WIDTH = 600;
const CHAMBER_WAVE_HEIGHT = 20;
const CHAMBER_WAVE_CENTER_Y = CHAMBER_WAVE_HEIGHT / 2;
const CHAMBER_WAVE_SEGMENTS = 64;
const CHAMBER_WAVE_CYCLE_MS = 2_160;
const CHAMBER_WAVE_PRIMARY_CYCLES = 1.35;
const CHAMBER_WAVE_SECONDARY_CYCLES = 2.4;
const CHAMBER_WAVE_SECONDARY_WEIGHT = 0.28;
const TIDE_ACTION_LOOP_MS: Record<TideAction, number> = {
  idle: 1_200,
  "running-right": 800,
  "running-left": 800,
  waving: 720,
  jumping: 750,
  failed: 1_040,
  waiting: 960,
  running: 840,
  review: 960,
};
const TIDE_ACTION_LOOPS: Record<TideAction, number> = {
  idle: 3,
  "running-right": 3,
  "running-left": 3,
  waving: 3,
  jumping: 3,
  failed: 2,
  waiting: 3,
  running: 3,
  review: 3,
};
const TIDE_ACTIONS: Record<StorySceneProps["snapshot"]["pressure"], readonly TideAction[]> = {
  safe: ["idle", "waving", "idle", "jumping", "idle"],
  warning: ["waiting", "idle", "review", "idle", "running"],
  danger: ["running-left", "idle", "running-right", "idle", "failed"],
  critical: ["failed", "idle", "waiting", "idle", "review"],
  recovery: ["waving", "idle", "jumping", "idle"],
};

function chamberWaveAmplitude(waterLevel: number): number {
  const usedFraction = Math.min(1, Math.max(0, waterLevel / 100));
  const edgeFactor = Math.min(
    1,
    Math.min(usedFraction, 1 - usedFraction) * 4,
  );
  return 4.8 * (0.35 + 0.65 * edgeFactor);
}

function chamberWavePath(phase: number, amplitude: number): string {
  const points = Array.from({ length: CHAMBER_WAVE_SEGMENTS + 1 }, (_, index) => {
    const x = (index / CHAMBER_WAVE_SEGMENTS) * CHAMBER_WAVE_WIDTH;
    const horizontalPosition = (x - CHAMBER_WAVE_WIDTH / 2) / CHAMBER_WAVE_WIDTH;
    const primaryWave = Math.sin(
      horizontalPosition * Math.PI * 2 * CHAMBER_WAVE_PRIMARY_CYCLES + phase,
    );
    const secondaryWave = Math.sin(
      horizontalPosition * Math.PI * 2 * CHAMBER_WAVE_SECONDARY_CYCLES - phase * 1.4,
    ) * CHAMBER_WAVE_SECONDARY_WEIGHT;
    const y = CHAMBER_WAVE_CENTER_Y + amplitude * (primaryWave + secondaryWave);
    return `${index === 0 ? "M" : "L"}${x.toFixed(2)} ${y.toFixed(2)}`;
  });
  return points.join(" ");
}

export function RisingWaterScene({ snapshot, displayMode }: StorySceneProps) {
  const { locale, text } = useI18n();
  const waterLevel = snapshot.weeklyUsed;
  const forecastLevel = Math.min(
    100,
    snapshot.projection?.projectedUsage ?? snapshot.weeklyUsed,
  );
  const state = pressureLabel(snapshot.pressure, locale);
  const isRecovery = snapshot.pressure === "recovery";
  const valveState = isRecovery
    ? text("重置阀已开启", "Reset valve open")
    : text("重置阀尚未解锁", "Reset valve locked");
  const projectionDescription =
    snapshot.projection === null
      ? text("预测样本不足", "Not enough samples to forecast")
      : text(
          `速率 ${snapshot.projection.rate}。${snapshot.projection.conclusion}`,
          `Rate ${snapshot.projection.rate}. ${snapshot.projection.conclusion}`,
        );
  const actions = TIDE_ACTIONS[snapshot.pressure];
  const [actionIndex, setActionIndex] = useState(0);
  const tideAction = actions[actionIndex % actions.length] ?? "idle";
  const waveFillRef = useRef<SVGPathElement>(null);
  const waveLineRef = useRef<SVGPathElement>(null);

  useEffect(() => {
    setActionIndex(0);
  }, [snapshot.pressure]);

  useEffect(() => {
    const reduceMotion =
      typeof window.matchMedia === "function" &&
      window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (reduceMotion || actions.length < 2) {
      return undefined;
    }
    const timeoutId = window.setTimeout(() => {
      setActionIndex((current) => (current + 1) % actions.length);
    }, TIDE_ACTION_LOOP_MS[tideAction] * TIDE_ACTION_LOOPS[tideAction]);
    return () => {
      window.clearTimeout(timeoutId);
    };
  }, [actions, tideAction]);

  const waterHeight = waterLevel * CHAMBER_WATER_CAP_RATIO;
  const forecastHeight = forecastLevel * CHAMBER_WATER_CAP_RATIO;
  const waveAmplitude = chamberWaveAmplitude(waterLevel);
  const initialWavePath = chamberWavePath(0, waveAmplitude);
  const liveWavePathRef = useRef(initialWavePath);

  useEffect(() => {
    const reduceMotion =
      typeof window.matchMedia === "function" &&
      window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (reduceMotion || typeof window.requestAnimationFrame !== "function") {
      return undefined;
    }

    let animationFrameId = 0;
    let startedAt: number | null = null;
    const animateWave = (timestamp: number) => {
      startedAt ??= timestamp;
      const elapsed = (timestamp - startedAt) % CHAMBER_WAVE_CYCLE_MS;
      const phase = (elapsed / CHAMBER_WAVE_CYCLE_MS) * Math.PI * 2;
      const linePath = chamberWavePath(phase, waveAmplitude);
      liveWavePathRef.current = linePath;
      waveLineRef.current?.setAttribute("d", linePath);
      waveFillRef.current?.setAttribute(
        "d",
        `${linePath} L${String(CHAMBER_WAVE_WIDTH)} ${String(CHAMBER_WAVE_HEIGHT)} L0 ${String(CHAMBER_WAVE_HEIGHT)} Z`,
      );
      animationFrameId = window.requestAnimationFrame(animateWave);
    };
    animationFrameId = window.requestAnimationFrame(animateWave);
    return () => {
      window.cancelAnimationFrame(animationFrameId);
    };
  }, [waveAmplitude]);

  return (
    <div
      class={`primary-stat quota-chamber pressure-${snapshot.pressure}`}
      data-story-theme="rising_water"
      data-story-display={displayMode}
      role="group"
      aria-label={text(
        `周额度压力舱：已用 ${snapshot.weeklyUsedLabel}，剩余 ${snapshot.weeklyRemainingLabel}，${state}。${snapshot.resetRelative}重置。${projectionDescription}`,
        `Weekly quota pressure chamber: ${snapshot.weeklyUsedLabel} used, ${snapshot.weeklyRemainingLabel} remaining, ${state}. Resets ${snapshot.resetRelative}. ${projectionDescription}`,
      )}
      title={projectionDescription}
      style={`--water-level:${String(waterLevel)}%;--water-height:${String(waterHeight)}%;--forecast-level:${String(forecastLevel)}%;--forecast-height:${String(forecastHeight)}%`}
    >
      <div class="quota-chamber__viewport" aria-hidden="true">
        <div class="quota-chamber__valve" title={valveState}>
          <span class="quota-chamber__valve-lock" />
        </div>
        {snapshot.projection === null ? null : (
          <div class="quota-chamber__forecast" />
        )}
        <div
          class={`quota-robot quota-robot--${snapshot.pressure} quota-robot--action-${tideAction}`}
          data-action={tideAction}
        >
          <span key={`${snapshot.pressure}-${String(actionIndex)}-${tideAction}`} class="quota-robot__sprite" />
        </div>
        <div class="quota-water">
          <span class="quota-water__wave" aria-hidden="true">
            <svg viewBox="0 0 600 20" preserveAspectRatio="none">
              <path
                ref={waveFillRef}
                class="quota-water__fill"
                d={`${liveWavePathRef.current} L${String(CHAMBER_WAVE_WIDTH)} ${String(CHAMBER_WAVE_HEIGHT)} L0 ${String(CHAMBER_WAVE_HEIGHT)} Z`}
              />
              <path
                ref={waveLineRef}
                class="quota-water__line"
                d={liveWavePathRef.current}
              />
            </svg>
          </span>
        </div>
        <span
          class="quota-chamber__reset-chip"
          title={snapshot.resetAbsolute}
        >
          {isRecovery ? text("排水中", "Draining") : snapshot.resetRelative}
        </span>
      </div>
    </div>
  );
}
