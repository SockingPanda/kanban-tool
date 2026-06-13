export const THEME_STORAGE_KEY = "kb:desktop:theme"

export type ThemeMode = "light" | "dark" | "system"
export type EffectiveTheme = "light" | "dark"

export function parseThemeMode(value: unknown): ThemeMode {
  return value === "light" || value === "dark" || value === "system" ? value : "system"
}

export function effectiveTheme(mode: ThemeMode, systemPrefersDark: boolean): EffectiveTheme {
  if (mode === "system") return systemPrefersDark ? "dark" : "light"
  return mode
}

export function applyRootTheme(root: Pick<DOMTokenList, "add" | "remove">, theme: EffectiveTheme) {
  if (theme === "dark") {
    root.add("dark")
  } else {
    root.remove("dark")
  }
}

export function nextThemeMode(mode: ThemeMode): ThemeMode {
  if (mode === "system") return "light"
  if (mode === "light") return "dark"
  return "system"
}
