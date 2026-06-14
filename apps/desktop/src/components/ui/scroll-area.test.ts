import { readFileSync } from "node:fs"
import { fileURLToPath } from "node:url"
import { describe, expect, it } from "vitest"

const sourceRoot = fileURLToPath(new URL("../../", import.meta.url))

function source(relativePath: string) {
  return readFileSync(new URL(relativePath, `file://${sourceRoot}`), "utf8")
}

describe("scroll area fade contract", () => {
  it("keeps shared Radix scrollbars mounted with fade classes", () => {
    const scrollArea = source("components/ui/scroll-area.tsx")

    expect(scrollArea).toContain('"relative min-h-0 overflow-hidden kb-scroll-area"')
    expect(scrollArea).toContain('"kb-scroll-area__scrollbar')
    expect(scrollArea).toContain('"kb-scroll-area__thumb')
    expect(scrollArea).toContain('type={type ?? "always"}')
  })

  it("defines hover, focus, drag, scroll, and reduced-motion fade states", () => {
    const styles = source("styles.css")

    expect(styles).toContain(".kb-scroll-area__scrollbar")
    expect(styles).toContain(".kb-scroll-area:hover .kb-scroll-area__scrollbar")
    expect(styles).toContain('.kb-scroll-area[data-scrolling="true"] .kb-scroll-area__scrollbar')
    expect(styles).toContain(".kb-scroll-area__scrollbar:active")
    expect(styles).toContain(".kb-native-scrollbar-fade")
    expect(styles).toContain(".kb-native-scrollbar-fade:hover")
    expect(styles).toContain('.kb-native-scrollbar-fade[data-scrolling="true"]')
    expect(styles).toContain("@media (prefers-reduced-motion: reduce)")
    expect(styles).toContain("scrollbar-color: var(--border) transparent")
  })
})
