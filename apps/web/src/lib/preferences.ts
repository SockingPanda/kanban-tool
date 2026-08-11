export type ThemeMode = "system" | "light" | "dark"
export type Locale = "zh" | "en"
export type DensityMode = "compact" | "comfortable"

export type WebPreferences = {
  theme: ThemeMode
  locale: Locale
  sidebarExpanded: boolean
  density: DensityMode
  /** User-selected actor preference; an empty value uses the host actor when transport integration consumes it. */
  actor: string
}

export const DEFAULT_PREFERENCES: WebPreferences = {
  theme: "system",
  locale: "zh",
  sidebarExpanded: true,
  density: "comfortable",
  actor: "",
}

export const PREFERENCE_STORAGE_KEYS = {
  theme: "kb:web:theme",
  locale: "kb:web:locale",
  sidebar: "kb:web:sidebar",
  density: "kb:web:density",
  actor: "kb:web:actor",
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

export function parseThemePreference(value: unknown): ThemeMode | null {
  return value === "system" || value === "light" || value === "dark" ? value : null
}

export function parseLocalePreference(value: unknown): Locale | null {
  return value === "zh" || value === "en" ? value : null
}

export function parseDensityPreference(value: unknown): DensityMode | null {
  return value === "compact" || value === "comfortable" ? value : null
}

const MAX_ACTOR_LENGTH = 128

/**
 * Validate the opaque actor name before a mutation transport integration consumes it.
 * The server accepts arbitrary non-empty text, but the Web surface rejects
 * control characters and bounds the value so it remains safe in headers and
 * diagnostics copied to the clipboard.
 */
export function parseActorPreference(value: unknown): string | null {
  if (typeof value !== "string") return null
  const actor = value.trim()
  if (actor.length === 0 || actor.length > MAX_ACTOR_LENGTH) return null
  for (const character of actor) {
    const code = character.codePointAt(0) ?? 0
    if (code <= 0x1f || code === 0x7f) return null
  }
  return actor
}

function validTheme(value: string | null): ThemeMode {
  return parseThemePreference(value) ?? DEFAULT_PREFERENCES.theme
}

function validLocale(value: string | null): Locale {
  return parseLocalePreference(value) ?? DEFAULT_PREFERENCES.locale
}

function validSidebar(value: string | null): boolean {
  return value === "collapsed" || value === "false" || value === "0" ? false : DEFAULT_PREFERENCES.sidebarExpanded
}

function validDensity(value: string | null): DensityMode {
  return parseDensityPreference(value) ?? DEFAULT_PREFERENCES.density
}

function validActor(value: string | null): string {
  return parseActorPreference(value) ?? DEFAULT_PREFERENCES.actor
}

export function readStoredPreferences(storage: PreferenceStorage | null = browserStorage()): WebPreferences {
  if (!storage) return DEFAULT_PREFERENCES
  try {
    return {
      theme: validTheme(storage.getItem(PREFERENCE_STORAGE_KEYS.theme)),
      locale: validLocale(storage.getItem(PREFERENCE_STORAGE_KEYS.locale)),
      sidebarExpanded: validSidebar(storage.getItem(PREFERENCE_STORAGE_KEYS.sidebar)),
      density: validDensity(storage.getItem(PREFERENCE_STORAGE_KEYS.density)),
      actor: validActor(storage.getItem(PREFERENCE_STORAGE_KEYS.actor)),
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
    storage.setItem(PREFERENCE_STORAGE_KEYS.density, preferences.density)
    // An empty actor intentionally remains in the Web namespace so the
    // preference shape is deterministic; transport falls back to runtime.actor.
    storage.setItem(PREFERENCE_STORAGE_KEYS.actor, preferences.actor)
  } catch {
    // Private browsing and disabled storage should not make the shell unusable.
  }
}

export function themeColorForMode(mode: ThemeMode, prefersDark = false): string {
  return mode === "dark" || (mode === "system" && prefersDark) ? "#1b1b1b" : "#f1f1f1"
}

export function loadingCopyForLocale(locale: Locale): string {
  return locale === "en" ? "Loading kanban-tool…" : "正在加载 kanban-tool…"
}

type PreferenceDocument = Pick<Document, "documentElement" | "querySelector">

export function applyWebPreferencesToDocument(preferences: WebPreferences, documentLike: PreferenceDocument): void {
  documentLike.documentElement.lang = preferences.locale === "en" ? "en" : "zh-CN"
  if (preferences.theme === "system") delete documentLike.documentElement.dataset.theme
  else documentLike.documentElement.dataset.theme = preferences.theme
  documentLike.documentElement.dataset.density = preferences.density
  const prefersDark = preferences.theme === "system"
    && typeof window !== "undefined"
    && typeof window.matchMedia === "function"
    && window.matchMedia("(prefers-color-scheme: dark)").matches
  documentLike.querySelector('meta[name="theme-color"]')?.setAttribute("content", themeColorForMode(preferences.theme, prefersDark))
  documentLike.querySelector("#bootstrap-loading-copy")?.replaceChildren(loadingCopyForLocale(preferences.locale))
}

/** Apply persisted shell preferences before the runtime bootstrap can fetch. */
export function prepareWebPreferences(documentLike?: PreferenceDocument): WebPreferences {
  const preferences = readStoredPreferences()
  const target = documentLike ?? (typeof document === "undefined" ? null : document)
  if (target) applyWebPreferencesToDocument(preferences, target)
  return preferences
}
