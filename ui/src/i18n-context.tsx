import { createContext, type ComponentChildren } from "preact";
import { useContext, useEffect, useMemo, useState } from "preact/hooks";

import {
  resolveInterfaceLocale,
  pseudoLocalize,
  translate,
  type InterfaceLocale,
  type InterfaceLocalePreference,
  type MessageKey,
} from "./i18n";

type I18nValue = {
  locale: InterfaceLocale;
  formatLocale: string;
  preference: InterfaceLocalePreference;
  text: (zhCn: string, english: string) => string;
  t: (
    key: MessageKey,
    args?: Readonly<Record<string, string | number>>,
  ) => string;
};

const defaultFormatLocale = "en";

const I18nContext = createContext<I18nValue>({
  locale: "zh-CN",
  formatLocale: defaultFormatLocale,
  preference: "zh-CN",
  text: (zhCn) => zhCn,
  t: (key, args) => translate("zh-CN", key, args),
});

function systemFormatLocale(): string {
  return navigator.languages[0] || navigator.language || defaultFormatLocale;
}

export function I18nProvider({
  preference,
  children,
}: {
  preference: InterfaceLocalePreference;
  children: ComponentChildren;
}) {
  const [systemLocale, setSystemLocale] = useState(systemFormatLocale);

  useEffect(() => {
    if (preference !== "system") {
      return;
    }
    const update = () => {
      setSystemLocale(systemFormatLocale());
    };
    window.addEventListener("languagechange", update);
    return () => {
      window.removeEventListener("languagechange", update);
    };
  }, [preference]);

  const locale = resolveInterfaceLocale(preference, systemLocale);
  const pseudo = new URLSearchParams(window.location.search).has("pseudo");
  useEffect(() => {
    document.documentElement.lang = locale;
  }, [locale]);

  const value = useMemo<I18nValue>(
    () => ({
      locale,
      formatLocale: systemLocale,
      preference,
      text: (zhCn, english) => {
        const value = locale === "zh-CN" ? zhCn : english;
        return pseudo ? pseudoLocalize(value) : value;
      },
      t: (key, args) => {
        const value = translate(locale, key, args);
        return pseudo ? pseudoLocalize(value) : value;
      },
    }),
    [locale, preference, pseudo, systemLocale],
  );

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useI18n(): I18nValue {
  return useContext(I18nContext);
}
