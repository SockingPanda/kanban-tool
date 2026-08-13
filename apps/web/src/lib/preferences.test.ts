import { describe, expect, test } from "vitest"

import {
  DEFAULT_PREFERENCES,
  DEFAULT_SIDEBAR_WIDTH_STEP,
  PREFERENCE_STORAGE_KEYS,
  parseActorPreference,
  parseDensityPreference,
  parseLocalePreference,
  parseSidebarWidthStep,
  parseThemePreference,
  readStoredPreferences,
  normalizeSidebarWidthStep,
  resetSidebarWidthPreference,
  sidebarWidthRem,
  shiftSidebarWidthStep,
  SIDEBAR_WIDTH_STEP_MAX,
  SIDEBAR_WIDTH_STEP_MIN,
  updateSidebarWidthPreference,
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
      [PREFERENCE_STORAGE_KEYS.sidebarWidth]: "63",
      [PREFERENCE_STORAGE_KEYS.density]: "compact",
      [PREFERENCE_STORAGE_KEYS.actor]: "web-user",
      unrelated: "must-not-be-read",
    })

    expect(readStoredPreferences(source)).toEqual({
      theme: "dark",
      locale: DEFAULT_PREFERENCES.locale,
      sidebarWidthStep: 63,
      density: "compact",
      actor: "web-user",
    })
  })

  test("writes all persisted preferences under kb:web keys", () => {
    const source = storage()
    const value: WebPreferences = {
      theme: "dark",
      locale: "en",
      sidebarWidthStep: 64,
      density: "compact",
      actor: "web-user",
    }

    writeStoredPreferences(source, value)

    expect([...source.values.entries()]).toEqual([
      [PREFERENCE_STORAGE_KEYS.theme, "dark"],
      [PREFERENCE_STORAGE_KEYS.locale, "en"],
      [PREFERENCE_STORAGE_KEYS.sidebarWidth, "64"],
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

  test("accepts only whole sidebar width steps in the persisted range", () => {
    expect(parseSidebarWidthStep(String(SIDEBAR_WIDTH_STEP_MIN))).toBe(SIDEBAR_WIDTH_STEP_MIN)
    expect(parseSidebarWidthStep(String(SIDEBAR_WIDTH_STEP_MAX))).toBe(SIDEBAR_WIDTH_STEP_MAX)
    expect(parseSidebarWidthStep("62.5")).toBeNull()
    expect(parseSidebarWidthStep("55")).toBeNull()
    expect(parseSidebarWidthStep("81")).toBeNull()
    expect(parseSidebarWidthStep(62.5)).toBeNull()
    expect(normalizeSidebarWidthStep(55)).toBe(SIDEBAR_WIDTH_STEP_MIN)
    expect(normalizeSidebarWidthStep(81)).toBe(SIDEBAR_WIDTH_STEP_MAX)
    expect(normalizeSidebarWidthStep(62.5)).toBe(DEFAULT_SIDEBAR_WIDTH_STEP)
    expect(sidebarWidthRem(DEFAULT_SIDEBAR_WIDTH_STEP)).toBe("15.5rem")
  })

  test("shifts width steps without depending on input events", () => {
    expect(shiftSidebarWidthStep(DEFAULT_SIDEBAR_WIDTH_STEP, 1)).toBe(63)
    expect(shiftSidebarWidthStep(SIDEBAR_WIDTH_STEP_MIN, -1)).toBe(SIDEBAR_WIDTH_STEP_MIN)
    expect(shiftSidebarWidthStep(SIDEBAR_WIDTH_STEP_MAX, 1)).toBe(SIDEBAR_WIDTH_STEP_MAX)
    expect(shiftSidebarWidthStep(62.5, 1)).toBe(DEFAULT_SIDEBAR_WIDTH_STEP + 1)
  })

  test("keeps sidebar width transitions pure and preserves other preferences", () => {
    const current = { ...DEFAULT_PREFERENCES, locale: "en" as const, actor: "web-user" }
    const updated = updateSidebarWidthPreference(current, 70)
    expect(updated).toEqual({ ...current, sidebarWidthStep: 70 })
    expect(resetSidebarWidthPreference(updated)).toEqual({ ...current, sidebarWidthStep: DEFAULT_SIDEBAR_WIDTH_STEP })
  })

  test("ignores the legacy collapse key and never writes it", () => {
    const source = storage({ "kb:web:sidebar": "collapsed" })
    expect(readStoredPreferences(source).sidebarWidthStep).toBe(DEFAULT_SIDEBAR_WIDTH_STEP)
    writeStoredPreferences(source, DEFAULT_PREFERENCES)
    expect(source.values.get("kb:web:sidebar")).toBe("collapsed")
    expect(source.values.get(PREFERENCE_STORAGE_KEYS.sidebarWidth)).toBe(String(DEFAULT_SIDEBAR_WIDTH_STEP))
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
