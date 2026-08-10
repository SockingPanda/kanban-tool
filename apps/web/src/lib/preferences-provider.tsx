import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react"

import {
  applyWebPreferencesToDocument,
  readStoredPreferences,
  writeStoredPreferences,
  type DensityMode,
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
    applyWebPreferencesToDocument(preferences, document)
  }, [preferences, storage])

  useEffect(() => {
    if (preferences.theme !== "system" || typeof window === "undefined" || typeof window.matchMedia !== "function") return
    const media = window.matchMedia("(prefers-color-scheme: dark)")
    const apply = () => {
      if (typeof document !== "undefined") applyWebPreferencesToDocument(preferences, document)
    }
    if (typeof media.addEventListener === "function") media.addEventListener("change", apply)
    else media.addListener?.(apply)
    return () => {
      if (typeof media.removeEventListener === "function") media.removeEventListener("change", apply)
      else media.removeListener?.(apply)
    }
  }, [preferences])

  const setTheme = useCallback((theme: ThemeMode) => setPreferences((current) => ({ ...current, theme })), [])
  const setLocale = useCallback((locale: Locale) => setPreferences((current) => ({ ...current, locale })), [])
  const setDensity = useCallback((density: DensityMode) => setPreferences((current) => ({ ...current, density })), [])
  const setActor = useCallback((actor: string) => setPreferences((current) => ({ ...current, actor })), [])
  const setSidebarExpanded = useCallback(
    (sidebarExpanded: boolean) => setPreferences((current) => ({ ...current, sidebarExpanded })),
    [],
  )
  const toggleSidebar = useCallback(() => {
    setPreferences((current) => ({ ...current, sidebarExpanded: !current.sidebarExpanded }))
  }, [])

  const value = useMemo(
    () => ({ ...preferences, setTheme, setLocale, setDensity, setActor, setSidebarExpanded, toggleSidebar }),
    [preferences, setActor, setDensity, setLocale, setSidebarExpanded, setTheme, toggleSidebar],
  )
  return <PreferencesContext.Provider value={value}>{children}</PreferencesContext.Provider>
}
