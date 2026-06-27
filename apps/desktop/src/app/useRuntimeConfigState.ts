import { useCallback, useEffect, useMemo, useState } from "react"

import {
  applyRootTheme,
  effectiveTheme,
  nextThemeMode,
  parseThemeMode,
  THEME_STORAGE_KEY,
  type ThemeMode,
} from "@/app/theme"
import { parseSidebarOpen, serializeSidebarOpen, SIDEBAR_OPEN_STORAGE_KEY } from "@/app/sidebar-state"
import { ApiError, KanbanApi, RuntimeConfig, loadRuntimeConfig } from "@/lib/api"

export function useRuntimeConfigState() {
  const [config, setConfig] = useState<RuntimeConfig | null>(null)
  const [themeMode, setThemeMode] = useState<ThemeMode>(() =>
    typeof window === "undefined" ? "system" : parseThemeMode(window.localStorage.getItem(THEME_STORAGE_KEY)),
  )
  const [sidebarOpen, setSidebarOpen] = useState(() =>
    typeof window === "undefined" ? true : parseSidebarOpen(window.localStorage.getItem(SIDEBAR_OPEN_STORAGE_KEY)),
  )
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    loadRuntimeConfig()
      .then(setConfig)
      .catch((err: unknown) => setError(errorMessage(err)))
  }, [])

  useEffect(() => {
    if (typeof window === "undefined") return
    const media = window.matchMedia("(prefers-color-scheme: dark)")
    const apply = () => applyRootTheme(document.documentElement.classList, effectiveTheme(themeMode, media.matches))
    apply()
    window.localStorage.setItem(THEME_STORAGE_KEY, themeMode)
    media.addEventListener("change", apply)
    return () => media.removeEventListener("change", apply)
  }, [themeMode])

  useEffect(() => {
    if (typeof window === "undefined") return
    window.localStorage.setItem(SIDEBAR_OPEN_STORAGE_KEY, serializeSidebarOpen(sidebarOpen))
  }, [sidebarOpen])

  const api = useMemo(() => (config ? new KanbanApi(config) : null), [config])
  const cycleThemeMode = useCallback(() => setThemeMode((current) => nextThemeMode(current)), [])

  return useMemo(
    () => ({
      api,
      config,
      setConfig,
      themeMode,
      setThemeMode,
      cycleThemeMode,
      sidebarOpen,
      setSidebarOpen,
      error,
      setError,
    }),
    [api, config, cycleThemeMode, error, sidebarOpen, themeMode],
  )
}

export function errorMessage(err: unknown) {
  if (err instanceof ApiError) return `${err.code}: ${err.message}`
  if (err instanceof Error) return err.message
  return String(err)
}
