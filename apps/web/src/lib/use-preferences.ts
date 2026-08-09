import { useContext } from "react"

import { PreferencesContext } from "./preferences-context"

export function usePreferences() {
  const preferences = useContext(PreferencesContext)
  if (!preferences) throw new Error("Preferences context is unavailable before bootstrap")
  return preferences
}
