import { useI18n } from "./i18n-context";

export type ColorTheme = "light" | "dark";

const STORAGE_KEY = "quotatide.theme";

function isColorTheme(value: string | null | undefined): value is ColorTheme {
  return value === "light" || value === "dark";
}

export function readInitialColorTheme(): ColorTheme {
  const queryTheme = new URLSearchParams(window.location.search).get("theme");
  if (isColorTheme(queryTheme)) {
    return queryTheme;
  }
  const documentTheme = document.documentElement.dataset.theme;
  if (isColorTheme(documentTheme)) {
    return documentTheme;
  }
  try {
    const storedTheme = window.localStorage.getItem(STORAGE_KEY);
    if (isColorTheme(storedTheme)) {
      return storedTheme;
    }
  } catch {
    // Storage can be unavailable in hardened WebViews. The system appearance
    // remains a complete fallback.
  }
  try {
    return window.matchMedia("(prefers-color-scheme: dark)").matches
      ? "dark"
      : "light";
  } catch {
    return "light";
  }
}

export function applyColorTheme(theme: ColorTheme): void {
  document.documentElement.dataset.theme = theme;
}

export function rememberColorTheme(theme: ColorTheme): void {
  try {
    window.localStorage.setItem(STORAGE_KEY, theme);
  } catch {
    // Theme switching still works for the current session.
  }
}

export function ThemeToggle({
  theme,
  onToggle,
}: {
  theme: ColorTheme;
  onToggle: () => void;
}) {
  const { text } = useI18n();
  const nextTheme = theme === "dark" ? "light" : "dark";

  return (
    <button
      type="button"
      class="theme-toggle"
      aria-label={
        nextTheme === "dark"
          ? text("切换到夜间模式", "Switch to dark mode")
          : text("切换到日间模式", "Switch to light mode")
      }
      title={
        nextTheme === "dark"
          ? text("夜间模式", "Dark mode")
          : text("日间模式", "Light mode")
      }
      onClick={onToggle}
    >
      {theme === "dark" ? (
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <circle cx="12" cy="12" r="4" />
          <path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4" />
        </svg>
      ) : (
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M20.2 15.7A8.5 8.5 0 0 1 8.3 3.8 8.5 8.5 0 1 0 20.2 15.7Z" />
        </svg>
      )}
    </button>
  );
}
