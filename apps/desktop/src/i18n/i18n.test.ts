import { describe, expect, it } from "vitest"

import {
  detectSystemLocale,
  parseLocaleMode,
  resolveLocale,
  setCurrentDesktopLocale,
  getCurrentDesktopLocale,
  translate,
} from "."

describe("desktop i18n", () => {
  it("parses persisted locale modes conservatively", () => {
    expect(parseLocaleMode("zh-CN")).toBe("zh-CN")
    expect(parseLocaleMode("en")).toBe("en")
    expect(parseLocaleMode("system")).toBe("system")
    expect(parseLocaleMode("fr-FR")).toBe("system")
    expect(parseLocaleMode(null)).toBe("system")
  })

  it("detects the first supported browser language", () => {
    expect(detectSystemLocale(["fr-FR", "en-US", "zh-CN"])).toBe("en")
    expect(detectSystemLocale(["zh-Hans-CN", "en-US"])).toBe("zh-CN")
    expect(detectSystemLocale(["C", "fr-FR"])).toBe("zh-CN")
  })

  it("resolves explicit and system locale modes", () => {
    expect(resolveLocale("en", ["zh-CN"])).toBe("en")
    expect(resolveLocale("zh-CN", ["en-US"])).toBe("zh-CN")
    expect(resolveLocale("system", ["en-US"])).toBe("en")
  })

  it("translates known messages and falls back to english keys", () => {
    expect(translate("zh-CN", "Search tasks")).toBe("搜索任务")
    expect(translate("zh-CN", "Open task {ref} {title}", { ref: "kanban-tool#1", title: "Demo" })).toBe("打开任务 kanban-tool#1 Demo")
    expect(translate("zh-CN", "Unmapped message")).toBe("Unmapped message")
    expect(translate("en", "Search tasks")).toBe("Search tasks")
  })

  it("keeps a process-wide locale for non-react request boundaries", () => {
    setCurrentDesktopLocale("en")
    expect(getCurrentDesktopLocale()).toBe("en")
    setCurrentDesktopLocale("zh-CN")
  })
})
