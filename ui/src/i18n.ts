export type InterfaceLocale = "zh-CN" | "en";
export type InterfaceLocalePreference = "system" | InterfaceLocale;

export const messages = {
  "zh-CN": {
    "language.system": "跟随系统",
    "language.zh-CN": "简体中文",
    "language.en": "English",
    "common.noData": "暂无数据",
    "time.resetSoon": "即将重置",
    "time.minuteFuture.one": "{count} 分钟后",
    "time.minuteFuture.other": "{count} 分钟后",
    "time.minutePast.one": "{count} 分钟前",
    "time.minutePast.other": "{count} 分钟前",
    "settings.language": "界面语言",
    "settings.languageHelp": "文案语言不会改变日期格式或额度自然日时区。",
  },
  en: {
    "language.system": "System",
    "language.zh-CN": "Simplified Chinese",
    "language.en": "English",
    "common.noData": "No data",
    "time.resetSoon": "Resetting soon",
    "time.minuteFuture.one": "in {count} minute",
    "time.minuteFuture.other": "in {count} minutes",
    "time.minutePast.one": "{count} minute ago",
    "time.minutePast.other": "{count} minutes ago",
    "settings.language": "Interface language",
    "settings.languageHelp":
      "The interface language does not change your date format or quota policy timezone.",
  },
} as const;

export type MessageKey = keyof (typeof messages)["en"];

export function resolveInterfaceLocale(
  preference: InterfaceLocalePreference,
  systemLocale: string | null,
): InterfaceLocale {
  if (preference !== "system") {
    return preference;
  }
  if (systemLocale === null) {
    return "en";
  }
  try {
    const locale = new Intl.Locale(systemLocale).maximize();
    if (locale.language === "zh") {
      if (locale.script === "Hant") {
        return "en";
      }
      if (
        locale.script === "Hans" ||
        locale.region === "CN" ||
        locale.region === "SG"
      ) {
        return "zh-CN";
      }
      return "en";
    }
    return locale.language === "en" ? "en" : "en";
  } catch {
    return "en";
  }
}

export function translate(
  locale: InterfaceLocale,
  key: MessageKey,
  args: Readonly<Record<string, string | number>> = {},
): string {
  let value: string = messages[locale][key];
  for (const [name, replacement] of Object.entries(args)) {
    value = value.replaceAll(`{${name}}`, String(replacement));
  }
  return value;
}

export function formatPercent(
  valueMicropoints: number | null,
  formatLocale: string,
): string {
  if (valueMicropoints === null || !Number.isFinite(valueMicropoints)) {
    return "";
  }
  return new Intl.NumberFormat(formatLocale, {
    style: "percent",
    minimumFractionDigits: 0,
    maximumFractionDigits: 1,
  }).format(valueMicropoints / 100_000_000);
}

export function formatResetTime(
  resetAtUnixMs: number | null,
  nowUnixMs: number,
  interfaceLocale: InterfaceLocale,
  formatLocale: string,
  policyTimezone: string,
): { absolute: string; accessible: string; relative: string } | null {
  if (
    resetAtUnixMs === null ||
    !Number.isFinite(resetAtUnixMs) ||
    !Number.isFinite(nowUnixMs)
  ) {
    return null;
  }
  const deltaMinutes = Math.trunc((resetAtUnixMs - nowUnixMs) / 60_000);
  const absolute = new Intl.DateTimeFormat(formatLocale, {
    dateStyle: "medium",
    timeStyle: "short",
    timeZone: policyTimezone,
  }).format(resetAtUnixMs);
  const relative =
    Math.abs(deltaMinutes) < 1
      ? translate(interfaceLocale, "time.resetSoon")
      : new Intl.RelativeTimeFormat(formatLocale, {
          numeric: "always",
          style: "long",
        }).format(deltaMinutes, "minute");
  const accessible =
    interfaceLocale === "zh-CN"
      ? `距离重置：${relative}；重置时间：${absolute}`
      : `Time until reset: ${relative}; reset time: ${absolute}`;
  return { absolute, accessible, relative };
}

export function pluralizedMinutes(
  locale: InterfaceLocale,
  count: number,
): string {
  const direction = count >= 0 ? "Future" : "Past";
  const absolute = Math.abs(count);
  const category = new Intl.PluralRules(locale).select(absolute);
  const suffix = category === "one" ? "one" : "other";
  const key = `time.minute${direction}.${suffix}` as MessageKey;
  return translate(locale, key, { count: absolute });
}

/** Development-only expansion helper used by `?pseudo=1` layout QA. */
export function pseudoLocalize(value: string): string {
  if (value.length === 0) {
    return value;
  }
  const expanded = value.replace(/[A-Za-z]/gu, (character) => {
    const accents: Readonly<Record<string, string>> = {
      a: "á",
      e: "ë",
      i: "ï",
      o: "ö",
      u: "ü",
      A: "Á",
      E: "Ë",
      I: "Ï",
      O: "Ö",
      U: "Ü",
    };
    return accents[character] ?? character;
  });
  return `⟦${expanded}${"·".repeat(Math.ceil(value.length * 0.4))}⟧`;
}
