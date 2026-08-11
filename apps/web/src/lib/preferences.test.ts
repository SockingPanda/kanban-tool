import { describe, expect, test } from "vitest"

import {
  DEFAULT_PREFERENCES,
  PREFERENCE_STORAGE_KEYS,
  parseActorPreference,
  parseDensityPreference,
  parseLocalePreference,
  parseThemePreference,
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
  test("defaults new users to the committed dark observation theme", () => {
    expect(DEFAULT_PREFERENCES.theme).toBe("dark")
    expect(readStoredPreferences(storage())).toMatchObject({ theme: "dark" })
  })

  test("reads only the kb:web namespace and falls back for invalid values", () => {
    const source = storage({
      [PREFERENCE_STORAGE_KEYS.theme]: "dark",
      [PREFERENCE_STORAGE_KEYS.locale]: "fr",
      [PREFERENCE_STORAGE_KEYS.sidebar]: "collapsed",
      [PREFERENCE_STORAGE_KEYS.density]: "compact",
      [PREFERENCE_STORAGE_KEYS.actor]: "web-user",
      unrelated: "must-not-be-read",
    })

    expect(readStoredPreferences(source)).toEqual({
      theme: "dark",
      locale: DEFAULT_PREFERENCES.locale,
      sidebarExpanded: false,
      density: "compact",
      actor: "web-user",
    })
  })

  test("writes all persisted preferences under kb:web keys", () => {
    const source = storage()
    const value: WebPreferences = {
      theme: "dark",
      locale: "en",
      sidebarExpanded: false,
      density: "compact",
      actor: "web-user",
    }

    writeStoredPreferences(source, value)

    expect([...source.values.entries()]).toEqual([
      [PREFERENCE_STORAGE_KEYS.theme, "dark"],
      [PREFERENCE_STORAGE_KEYS.locale, "en"],
      [PREFERENCE_STORAGE_KEYS.sidebar, "collapsed"],
      [PREFERENCE_STORAGE_KEYS.density, "compact"],
      [PREFERENCE_STORAGE_KEYS.actor, "web-user"],
    ])
  })

  test("parses only supported theme and locale values", () => {
    expect(parseThemePreference("dark")).toBe("dark")
    expect(parseThemePreference("system")).toBe("system")
    expect(parseLocalePreference("en")).toBe("en")
    expect(parseLocalePreference("fr")).toBeNull()
    expect(parseDensityPreference("compact")).toBe("compact")
    expect(parseDensityPreference("spacious")).toBeNull()
  })

  test("accepts bounded actor identities and rejects control or empty values", () => {
    expect(parseActorPreference(" web-user ")).toBe("web-user")
    expect(parseActorPreference(" ")).toBeNull()
    expect(parseActorPreference("web\nuser")).toBeNull()
    expect(parseActorPreference("a".repeat(129))).toBeNull()
  })

  test("survives hostile storage reads", () => {
    const hostile = {
      getItem: () => {
        throw new Error("storage blocked")
      },
      setItem: () => {
        throw new Error("storage blocked")
      },
    }
    expect(readStoredPreferences(hostile)).toEqual(DEFAULT_PREFERENCES)
    expect(() => writeStoredPreferences(hostile, DEFAULT_PREFERENCES)).not.toThrow()
  })
})
