import { describe, expect, test } from "vitest"

import {
  DEFAULT_PREFERENCES,
  PREFERENCE_STORAGE_KEYS,
  readStoredPreferences,
  writeStoredPreferences,
  type WebPreferences,
} from "./preferences"

function storage(values: Record<string, string> = {}) {
  const data = new Map(Object.entries(values))
  return {
    getItem: (key: string) => data.get(key) ?? null,
    setItem: (key: string, value: string) => data.set(key, value),
    values: data,
  }
}

describe("Web preferences", () => {
  test("reads only the kb:web namespace and falls back for invalid values", () => {
    const source = storage({
      [PREFERENCE_STORAGE_KEYS.theme]: "dark",
      [PREFERENCE_STORAGE_KEYS.locale]: "fr",
      [PREFERENCE_STORAGE_KEYS.sidebar]: "collapsed",
      unrelated: "must-not-be-read",
    })

    expect(readStoredPreferences(source)).toEqual({
      theme: "dark",
      locale: DEFAULT_PREFERENCES.locale,
      sidebarExpanded: false,
    })
  })

  test("writes all persisted preferences under kb:web keys", () => {
    const source = storage()
    const value: WebPreferences = { theme: "dark", locale: "en", sidebarExpanded: false }

    writeStoredPreferences(source, value)

    expect([...source.values.entries()]).toEqual([
      [PREFERENCE_STORAGE_KEYS.theme, "dark"],
      [PREFERENCE_STORAGE_KEYS.locale, "en"],
      [PREFERENCE_STORAGE_KEYS.sidebar, "collapsed"],
    ])
  })
})
