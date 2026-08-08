import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react"

import {
  readStoredPreferences,
  themeColorForMode,
  writeStoredPreferences,
  type Locale,
  type ThemeMode,
  type WebPreferences,
} from "./preferences"
import { PreferencesContext } from "./preferences-context"

export function PreferencesProvider({ children }: { children: ReactNode }) {
  const [preferences, setPreferences] = useState<WebPreferences>(() => readStoredPreferences())
  const storage = useMemo(
    () =>
      typeof window === "undefined"
        ? null
        : (() => {
            try {
              return window.localStorage
            } catch {
              return null
            }
          })(),
    [],
  )

  useEffect(() => {
    writeStoredPreferences(storage, preferences)
    if (typeof document === "undefined") return
    document.documentElement.lang = preferences.locale === "en" ? "en" : "zh-CN"
    document.documentElement.dataset.theme = preferences.theme
    document.querySelector('meta[name="theme-color"]')?.setAttribute("content", themeColorForMode(preferences.theme))
  }, [preferences, storage])

  const setTheme = useCallback((theme: ThemeMode) => setPreferences((current) => ({ ...current, theme })), [])
  const setLocale = useCallback((locale: Locale) => setPreferences((current) => ({ ...current, locale })), [])
  const setSidebarExpanded = useCallback(
    (sidebarExpanded: boolean) => setPreferences((current) => ({ ...current, sidebarExpanded })),
    [],
  )
  const toggleSidebar = useCallback(() => {
    setPreferences((current) => ({ ...current, sidebarExpanded: !current.sidebarExpanded }))
  }, [])

  const value = useMemo(
    () => ({ ...preferences, setTheme, setLocale, setSidebarExpanded, toggleSidebar }),
    [preferences, setLocale, setSidebarExpanded, setTheme, toggleSidebar],
  )
  return <PreferencesContext.Provider value={value}>{children}</PreferencesContext.Provider>
}
