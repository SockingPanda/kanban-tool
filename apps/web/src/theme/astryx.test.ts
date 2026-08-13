import { readFileSync } from "node:fs"

import { describe, expect, test } from "vitest"

const stylesheet = readFileSync(new URL("./astryx.css", import.meta.url), "utf8")

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\\]\\]/g, "\\$&")
}

function lightDarkToken(name: string): readonly [string, string] {
  const match = stylesheet.match(
    new RegExp(`${escapeRegExp(name)}:\\s*light-dark\\((#[0-9a-f]{6}),\\s*(#[0-9a-f]{6})\\)\\s*;`, "i"),
  )
  if (match === null) throw new Error(`Missing generated light-dark token: ${name}`)
  return [match[1].toLowerCase(), match[2].toLowerCase()]
}

function relativeLuminance(hex: string): number {
  const channels = hex.slice(1).match(/../g)
  if (channels === null || channels.length !== 3) throw new Error(`Invalid color token: ${hex}`)
  const linear = channels.map((channel) => {
    const value = Number.parseInt(channel, 16) / 255
    return value <= 0.03928 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4
  })
  return 0.2126 * linear[0] + 0.7152 * linear[1] + 0.0722 * linear[2]
}

function contrastRatio(foreground: string, background: string): number {
  const foregroundLuminance = relativeLuminance(foreground)
  const backgroundLuminance = relativeLuminance(background)
  const lighter = Math.max(foregroundLuminance, backgroundLuminance)
  const darker = Math.min(foregroundLuminance, backgroundLuminance)
  return (lighter + 0.05) / (darker + 0.05)
}

describe("generated Astryx accent text tokens", () => {
  test("keeps active and selected regular text at WCAG AA in both themes", () => {
    const textAccent = lightDarkToken("--color-text-accent")
    const accentMuted = lightDarkToken("--color-accent-muted")
    const body = lightDarkToken("--color-background-body")

    for (const mode of [0, 1] as const) {
      expect(contrastRatio(textAccent[mode], accentMuted[mode])).toBeGreaterThanOrEqual(4.5)
      expect(contrastRatio(textAccent[mode], body[mode])).toBeGreaterThanOrEqual(4.5)
    }
  })

  test("keeps the stronger text accent separate from the filled action accent", () => {
    expect(lightDarkToken("--color-text-accent")[0]).not.toBe(lightDarkToken("--color-accent")[0])
  })
})
