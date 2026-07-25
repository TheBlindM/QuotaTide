export function localDateParts(date, timezone) {
  const parts = new Intl.DateTimeFormat("en-CA", {
    timeZone: timezone,
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    weekday: "short",
  }).formatToParts(date);
  const value = (type) => parts.find((part) => part.type === type)?.value;
  return {
    date: `${value("year")}-${value("month")}-${value("day")}`,
    weekday: value("weekday"),
  };
}

export function dailyLimitFor(date, timezone) {
  const { weekday } = localDateParts(date, timezone);
  return weekday === "Sat" || weekday === "Sun" ? 10 : 16;
}

export function evaluateDailyPolicy(used, limit) {
  const safeUsed = Math.max(0, Number(used) || 0);
  const ratio = limit > 0 ? safeUsed / limit : 0;
  let status = "normal";
  if (ratio >= 1) status = "exceeded";
  else if (ratio >= 0.8) status = "warning";
  return {
    used: safeUsed,
    limit,
    ratio,
    progressPercent: Math.min(100, ratio * 100),
    status,
  };
}

export const RESET_AT_JITTER_TOLERANCE_SECONDS = 60;

export function isEpochChange(previous, current) {
  if (!previous) return false;
  const resetAtChanged =
    Number.isFinite(previous.resetAt) &&
    Number.isFinite(current.resetAt) &&
    Math.abs(previous.resetAt - current.resetAt) >
      RESET_AT_JITTER_TOLERANCE_SECONDS;
  const usageDropped =
    current.usedPercent + 0.01 < previous.usedPercent;
  return resetAtChanged || usageDropped;
}

export function calculateDelta(previous, current) {
  if (!previous) return 0;
  return isEpochChange(previous, current)
    ? Math.max(0, current.usedPercent)
    : Math.max(0, current.usedPercent - previous.usedPercent);
}
