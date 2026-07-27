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

export const BASE_WORKDAY_LIMIT = 16;
export const WEEKDAY_BASE_TOTAL = BASE_WORKDAY_LIMIT * 5;
export const WEEKEND_DAILY_LIMIT = 10;

export function dayPolicyFor(localDate) {
  const weekday = new Date(`${localDate}T00:00:00Z`).getUTCDay();
  return weekday === 0 || weekday === 6
    ? { kind: "weekend_fixed", baseLimit: WEEKEND_DAILY_LIMIT }
    : { kind: "weekday_dynamic", baseLimit: BASE_WORKDAY_LIMIT };
}

export function dailyLimitFor(date, timezone) {
  const { date: localDate } = localDateParts(date, timezone);
  return dayPolicyFor(localDate).baseLimit;
}

function addLocalDays(localDate, days) {
  const date = new Date(`${localDate}T00:00:00Z`);
  date.setUTCDate(date.getUTCDate() + days);
  return date.toISOString().slice(0, 10);
}

export function dynamicDailyLimitFor(localDate, usageByDate) {
  const weekday = new Date(`${localDate}T00:00:00Z`).getUTCDay();
  const policy = dayPolicyFor(localDate);
  if (policy.kind === "weekend_fixed") return policy.baseLimit;

  const monday = addLocalDays(localDate, -(weekday - 1));
  let carryover = 0;
  for (let offset = 0; offset < weekday - 1; offset += 1) {
    const date = addLocalDays(monday, offset);
    const remainingWorkdays = 5 - offset;
    const allocatedCarryover = carryover / remainingWorkdays;
    const limit = policy.baseLimit + allocatedCarryover;
    const unused = usageByDate.has(date)
      ? Math.max(0, limit - Math.max(0, Number(usageByDate.get(date)) || 0))
      : 0;
    carryover = carryover - allocatedCarryover + unused;
  }
  const remainingWorkdays = 6 - weekday;
  return policy.baseLimit + carryover / remainingWorkdays;
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
