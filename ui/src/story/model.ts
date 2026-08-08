import type { QuotaPressure } from "../bindings/QuotaPressure";
import type { InterfaceLocale } from "../i18n";

export type StorySource = Readonly<{
  weeklyUsed: string;
  weeklyRemaining: string;
  pressure: QuotaPressure;
  burnProjection: Readonly<{
    rate: string;
    projectedUsage: string;
    conclusion: string;
  }> | null;
  resetAbsolute: string;
  resetRelative: string;
  radar: Readonly<{
    kind: string;
    chance?: string;
  }> | null;
}>;

export type StorySnapshot = Readonly<{
  weeklyUsed: number;
  weeklyUsedLabel: string;
  weeklyRemaining: number;
  weeklyRemainingLabel: string;
  pressure: QuotaPressure;
  projection: Readonly<{
    rate: string;
    projectedUsage: number;
    projectedUsageLabel: string;
    conclusion: string;
  }> | null;
  resetAbsolute: string;
  resetRelative: string;
  radarChance: string | null;
}>;

export function percentValue(value: string): number {
  const parsed = Number.parseFloat(value.replace("%", "").replace(",", "."));
  return Number.isFinite(parsed) ? Math.min(100, Math.max(0, parsed)) : 0;
}

export function pressureLabel(
  pressure: QuotaPressure,
  locale: InterfaceLocale,
): string {
  const labels: Record<QuotaPressure, readonly [string, string]> = {
    safe: ["安全", "Safe"],
    warning: ["提醒", "Warning"],
    danger: ["高压", "Danger"],
    critical: ["临界", "Critical"],
    recovery: ["恢复", "Recovery"],
  };
  const [zh, en] = labels[pressure];
  return locale === "zh-CN" ? zh : en;
}

export function createStorySnapshot(source: StorySource): StorySnapshot {
  return {
    weeklyUsed: percentValue(source.weeklyUsed),
    weeklyUsedLabel: source.weeklyUsed,
    weeklyRemaining: percentValue(source.weeklyRemaining),
    weeklyRemainingLabel: source.weeklyRemaining,
    pressure: source.pressure,
    projection:
      source.burnProjection === null
        ? null
        : {
            rate: source.burnProjection.rate,
            projectedUsage: percentValue(source.burnProjection.projectedUsage),
            projectedUsageLabel: source.burnProjection.projectedUsage,
            conclusion: source.burnProjection.conclusion,
          },
    resetAbsolute: source.resetAbsolute,
    resetRelative: source.resetRelative,
    radarChance:
      source.radar?.kind === "active" ? source.radar.chance ?? null : null,
  };
}
