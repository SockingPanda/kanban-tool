import { readFileSync } from "node:fs"
import { describe, expect, test } from "vitest"

function source(path: string): string {
  return readFileSync(new URL(path, import.meta.url), "utf8")
}

describe("formal 09C browser context contract", () => {
  test("aggregates the same dark/astryx context emitted by the proof specs", () => {
    const aggregate = source("../../../scripts/a11y-visual-proof-09c.sh")
    const keyboard = source("../tests/a11y-keyboard.spec.ts")
    const visual = source("../tests/a11y-visual.spec.ts")

    expect(aggregate.match(/\.context\.theme_state == "dark"/g)).toHaveLength(2)
    expect(aggregate.match(/\.context\.astryx_theme == "astryx"/g)).toHaveLength(2)
    expect(aggregate).not.toContain('.context.theme_state == "system"')
    expect(aggregate).not.toContain('.context.astryx_theme == "neutral"')
    expect(keyboard).toContain('context.theme_state === "dark"')
    expect(keyboard).toContain('context.astryx_theme === "astryx"')
    expect(visual).toContain('context.theme_state === "dark"')
    expect(visual).toContain('context.astryx_theme === "astryx"')
  })
})
