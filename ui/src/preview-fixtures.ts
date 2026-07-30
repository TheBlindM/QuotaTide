import type { PublicLedgerDay } from "./bindings/PublicLedgerDay";
import type { PublicLiveQuota } from "./bindings/PublicLiveQuota";
import type { PublicResetRadar } from "./bindings/PublicResetRadar";
import type { InterfaceLocale } from "./i18n";
import type { LedgerTone } from "./WeeklyLedger";

export const PREVIEW_NOW_UNIX_MS = Date.parse(
  "2026-07-28T10:01:00+08:00",
);

const PREVIEW_WINDOW_START_UNIX_S =
  Date.parse("2026-07-24T10:01:00+08:00") / 1000;
const PREVIEW_RESET_UNIX_S =
  Date.parse("2026-07-31T10:01:00+08:00") / 1000;

export type PreviewScenario = {
  configured: boolean;
  formatLocale: string;
  interfaceLocale: InterfaceLocale;
  liveQuota: PublicLiveQuota | null;
  radar: PublicResetRadar;
  tone: LedgerTone;
};

export function createPreviewScenario(
  searchParams: URLSearchParams,
): PreviewScenario {
  const tone = parseTone(searchParams.get("state"));
  const interfaceLocale: InterfaceLocale =
    searchParams.get("lang") === "en" ? "en" : "zh-CN";
  return {
    configured: tone !== "unconfigured",
    formatLocale: parseFormatLocale(
      searchParams.get("format"),
      interfaceLocale,
    ),
    interfaceLocale,
    liveQuota: createLiveQuota(tone),
    radar: createRadar(searchParams.get("radar") === "active"),
    tone,
  };
}

function parseTone(value: string | null): LedgerTone {
  return value === "warning" ||
    value === "over" ||
    value === "stale" ||
    value === "unconfigured"
    ? value
    : "fresh";
}

function parseFormatLocale(
  value: string | null,
  interfaceLocale: InterfaceLocale,
): string {
  if (value !== null) {
    try {
      return new Intl.Locale(value).toString();
    } catch {
      // Preview query strings are untrusted development input.
    }
  }
  return interfaceLocale === "en" ? "en-US" : "zh-CN";
}

function ledgerDay(
  localDate: string,
  usedMicropoints: number | null,
  baseMicropoints: number,
  carryMicropoints: number,
  isToday: boolean,
  status: PublicLedgerDay["status"],
): PublicLedgerDay {
  return {
    localDate,
    usedMicropoints,
    policyRevision: 1,
    policyTimezone: "Asia/Shanghai",
    baseMicropoints,
    carryMicropoints,
    limitMicropoints: baseMicropoints + carryMicropoints,
    isToday,
    finalized: status === "finalized",
    status,
  };
}

function createLiveQuota(tone: LedgerTone): PublicLiveQuota | null {
  if (tone === "unconfigured") {
    return null;
  }
  const stale = tone === "stale";
  const todayUsedMicropoints =
    tone === "over"
      ? 18_200_000
      : tone === "warning"
        ? 14_200_000
        : 11_400_000;
  const usedMicropoints =
    tone === "over" ? 52_000_000 : tone === "warning" ? 45_000_000 : 42_000_000;
  return {
    usedMicropoints,
    remainingMicropoints: 100_000_000 - usedMicropoints,
    capturedAtUnixMs: PREVIEW_NOW_UNIX_MS,
    resetsAtUnixS: PREVIEW_RESET_UNIX_S,
    windowStartsAtUnixS: PREVIEW_WINDOW_START_UNIX_S,
    windowEndsAtUnixS: PREVIEW_RESET_UNIX_S - 1,
    planType: "plus",
    allowed: true,
    lastAttemptAtUnixMs: PREVIEW_NOW_UNIX_MS,
    lastSuccessAtUnixMs: stale
      ? PREVIEW_NOW_UNIX_MS - 3 * 60 * 60 * 1000
      : PREVIEW_NOW_UNIX_MS,
    consecutiveFailures: stale ? 3 : 0,
    sourceStatus: stale ? "stale_after_failure" : "fresh",
    publicError: stale ? "timeout" : null,
    todayBaseMicropoints: 16_000_000,
    todayCarryMicropoints: 800_000,
    todayLimitMicropoints: 16_800_000,
    todayAvailableMicropoints: Math.max(
      0,
      16_800_000 - todayUsedMicropoints,
    ),
    ledgerDays: [
      ledgerDay(
        "2026-07-24",
        12_800_000,
        16_000_000,
        0,
        false,
        "finalized",
      ),
      ledgerDay(
        "2026-07-25",
        6_000_000,
        10_000_000,
        0,
        false,
        "finalized",
      ),
      ledgerDay(
        "2026-07-26",
        1_000_000,
        10_000_000,
        0,
        false,
        "finalized",
      ),
      ledgerDay(
        "2026-07-27",
        11_000_000,
        16_000_000,
        0,
        false,
        "finalized",
      ),
      ledgerDay(
        "2026-07-28",
        todayUsedMicropoints,
        16_000_000,
        800_000,
        true,
        tone === "over"
          ? "exceeded"
          : tone === "warning"
            ? "warning"
            : "normal",
      ),
      ledgerDay(
        "2026-07-29",
        null,
        16_000_000,
        800_000,
        false,
        "unknown",
      ),
      ledgerDay(
        "2026-07-30",
        null,
        16_000_000,
        400_000,
        false,
        "unknown",
      ),
    ],
  };
}

function createRadar(active: boolean): PublicResetRadar {
  if (!active) {
    return {
      lastAttemptAtUnixMs: null,
      lastSuccessAtUnixMs: null,
      consecutiveFailures: 0,
      sourceStatus: "unavailable",
      publicError: null,
      prediction: null,
      latestAnnouncement: null,
    };
  }
  return {
    lastAttemptAtUnixMs: PREVIEW_NOW_UNIX_MS,
    lastSuccessAtUnixMs: PREVIEW_NOW_UNIX_MS,
    consecutiveFailures: 0,
    sourceStatus: "fresh",
    publicError: null,
    prediction: {
      chanceBasisPoints: 7_500,
      displayChance: ">70%",
      observedAtUnixMs: PREVIEW_NOW_UNIX_MS - 60 * 60 * 1000,
      expiresAtUnixMs: PREVIEW_NOW_UNIX_MS + 23 * 60 * 60 * 1000,
      explanation: "I'm feeling like a limit reset.",
      sourceUrl: "https://x.com/thsottiaux/status/2081899343091843463",
    },
    latestAnnouncement: {
      announcedAtUnixMs: PREVIEW_NOW_UNIX_MS - 22 * 60 * 60 * 1000,
      text: "Codex limits were reset.",
      sourceUrl: "https://x.com/thsottiaux/status/2082317452755751098",
    },
  };
}
