import { createContext } from "react"

import type { DensityMode, Locale, ThemeMode, WebPreferences } from "./preferences"

export type PreferencesContextValue = WebPreferences & {
  density: DensityMode
  actor: string
  setTheme: (theme: ThemeMode) => void
  setLocale: (locale: Locale) => void
  setDensity: (density: DensityMode) => void
  setActor: (actor: string) => void
  setSidebarExpanded: (expanded: boolean) => void
  toggleSidebar: () => void
}

export const PreferencesContext = createContext<PreferencesContextValue | null>(null)
