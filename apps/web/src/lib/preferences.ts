export type ThemeMode = "light" | "dark"
export type Locale = "zh" | "en"

export type WebPreferences = {
  theme: ThemeMode
  locale: Locale
  sidebarExpanded: boolean
}

export const DEFAULT_PREFERENCES: WebPreferences = {
  theme: "light",
  locale: "zh",
  sidebarExpanded: true,
}

export const PREFERENCE_STORAGE_KEYS = {
  theme: "kb:web:theme",
  locale: "kb:web:locale",
  sidebar: "kb:web:sidebar",
} as const

type PreferenceStorage = Pick<Storage, "getItem" | "setItem">

function browserStorage(): PreferenceStorage | null {
  if (typeof window === "undefined") return null
  try {
    return window.localStorage
  } catch {
    return null
  }
}

function validTheme(value: string | null): ThemeMode {
  return value === "dark" ? "dark" : DEFAULT_PREFERENCES.theme
}

function validLocale(value: string | null): Locale {
  return value === "en" ? "en" : DEFAULT_PREFERENCES.locale
}

function validSidebar(value: string | null): boolean {
  return value === "collapsed" || value === "false" || value === "0" ? false : DEFAULT_PREFERENCES.sidebarExpanded
}

export function readStoredPreferences(storage: PreferenceStorage | null = browserStorage()): WebPreferences {
  if (!storage) return DEFAULT_PREFERENCES
  try {
    return {
      theme: validTheme(storage.getItem(PREFERENCE_STORAGE_KEYS.theme)),
      locale: validLocale(storage.getItem(PREFERENCE_STORAGE_KEYS.locale)),
      sidebarExpanded: validSidebar(storage.getItem(PREFERENCE_STORAGE_KEYS.sidebar)),
    }
  } catch {
    return DEFAULT_PREFERENCES
  }
}

export function writeStoredPreferences(storage: PreferenceStorage | null, preferences: WebPreferences): void {
  if (!storage) return
  try {
    storage.setItem(PREFERENCE_STORAGE_KEYS.theme, preferences.theme)
    storage.setItem(PREFERENCE_STORAGE_KEYS.locale, preferences.locale)
    storage.setItem(PREFERENCE_STORAGE_KEYS.sidebar, preferences.sidebarExpanded ? "expanded" : "collapsed")
  } catch {
    // Private browsing and disabled storage should not make the shell unusable.
  }
}

export function themeColorForMode(mode: ThemeMode): string {
  return mode === "dark" ? "#1b1b1b" : "#f1f1f1"
}
