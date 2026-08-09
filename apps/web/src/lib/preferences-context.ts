import { createContext } from "react"

import type { Locale, ThemeMode, WebPreferences } from "./preferences"

export type PreferencesContextValue = WebPreferences & {
  setTheme: (theme: ThemeMode) => void
  setLocale: (locale: Locale) => void
  setSidebarExpanded: (expanded: boolean) => void
  toggleSidebar: () => void
}

export const PreferencesContext = createContext<PreferencesContextValue | null>(null)
