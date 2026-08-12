import { mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs"
import os from "node:os"
import path from "node:path"

import tailwindcss from "@tailwindcss/vite"
import { build as viteBuild } from "vite"
import { describe, expect, test } from "vitest"

import { readUiGuardManifest, scanUiSource, uiGuardModeFromEnv, validateUiGuardManifest } from "./ui-guard"

const projectRoot = path.resolve(import.meta.dirname, "..")

describe("Astryx CSP UI guard", () => {
  test("loads the machine-readable manifest and shared page contract", () => {
    const manifest = readUiGuardManifest()
    expect(manifest.schemaVersion).toBe(1)
    expect(manifest.tailwindBridge.version).toBe("4.3.3")
    expect(manifest.pageContract.sharedStyleEntry).toBe("src/styles.css")
    expect(manifest.pageContract.entrypoints).toEqual(expect.arrayContaining([
      { path: "src/main.tsx", import: "./styles.css" },
      { path: ".storybook/preview.tsx", import: "../src/styles.css" },
    ]))
    expect(() => validateUiGuardManifest(manifest)).not.toThrow()
  })

  test("inventories the current source without traversing reference/output", () => {
    const report = scanUiSource({ projectRoot, sourcePaths: ["src", ".storybook", "vite.config.ts"] })
    expect(report.passed).toBe(true)
    expect(report.files.some((file) => file.includes(`${path.sep}stories${path.sep}reference${path.sep}`))).toBe(false)
    expect(report.warnings.some((warning) => warning.code === "residual-css")).toBe(true)
  })

  test("fails closed for unsafe source imports, inline style, DOM style, and forbidden paths", () => {
    const temporaryRoot = mkdtempSync(path.join(os.tmpdir(), "kanban-ui-guard-"))
    try {
      writeFileSync(path.join(temporaryRoot, "src.tsx"), [
        'import "@astryxdesign/core/TextInput"',
        'import { TextInput as UnsafeInput } from "@astryxdesign/core"',
        'import "./reference/example"',
        'import "./lib/preferences"',
        'export function Bad() { return <div style={{ color: "red" }} /> }',
        'declare const ordinary: { style: { color: string } }',
        'void ordinary.style.color',
        'document.body.style.color = "red"',
        'Object.assign(document.body.style, { color: "blue" })',
      ].join("\n"))
      const report = scanUiSource({ projectRoot: temporaryRoot, sourcePaths: ["src.tsx"], mode: "enforce" })
      expect(report.passed).toBe(false)
      expect(report.errors.map((error) => error.code)).toEqual(expect.arrayContaining([
        "unsafe-direct-import",
        "forbidden-import",
        "inline-style",
        "dom-style",
      ]))
      expect(report.errors.filter((error) => error.code === "unsafe-direct-import")).toHaveLength(2)
      expect(report.errors.filter((error) => error.code === "forbidden-import")).toHaveLength(1)
      expect(report.errors.filter((error) => error.code === "dom-style")).toHaveLength(2)
    } finally {
      rmSync(temporaryRoot, { recursive: true, force: true })
    }
  })

  test("resolves only the supported inventory and enforce modes", () => {
    expect(uiGuardModeFromEnv(undefined)).toBe("inventory")
    expect(uiGuardModeFromEnv("inventory")).toBe("inventory")
    expect(uiGuardModeFromEnv("enforce")).toBe("enforce")
    expect(() => uiGuardModeFromEnv("strict")).toThrow(/inventory or enforce/)
    const enforce = scanUiSource({ projectRoot, sourcePaths: ["src/ui/foundations/tokens.css"], mode: "enforce" })
    expect(enforce.errors.some((error) => error.code === "residual-css")).toBe(true)
  })

  test("keeps raw layout within its manifest baseline", () => {
    const temporaryRoot = mkdtempSync(path.join(os.tmpdir(), "kanban-ui-layout-"))
    try {
      writeFileSync(path.join(temporaryRoot, "semantic.tsx"), 'export const MachineLabel = () => <span translate="no">KB-42</span>')
      const semantic = scanUiSource({ projectRoot: temporaryRoot, sourcePaths: ["semantic.tsx"], mode: "enforce" })
      expect(semantic.errors.some((error) => error.code.startsWith("legacy-raw-layout"))).toBe(false)

      writeFileSync(path.join(temporaryRoot, "src.tsx"), [
        'export const AddedLayout = () => <div className="flex"><span>semantic</span><span className="flex">layout</span></div>',
      ].join("\n"))
      const inventory = scanUiSource({ projectRoot: temporaryRoot, sourcePaths: ["src.tsx"] })
      expect(inventory.errors.some((error) => error.code === "legacy-raw-layout-over-budget")).toBe(true)
      const enforce = scanUiSource({ projectRoot: temporaryRoot, sourcePaths: ["src.tsx"], mode: "enforce" })
      expect(enforce.errors.some((error) => error.code === "legacy-raw-layout-over-budget")).toBe(true)
    } finally {
      rmSync(temporaryRoot, { recursive: true, force: true })
    }
  })

  test("the shared CSS entry carries Tailwind utilities and Astryx layers", () => {
    const source = readFileSync(path.join(projectRoot, "src/styles.css"), "utf8")
    expect(source).toContain('@import "tailwindcss/utilities.css" layer(utilities);')
    expect(source).toContain('@import "@astryxdesign/core/astryx.css";')
    expect(source).toContain('@import "@astryxdesign/core/tailwind-theme.css";')
    expect(readFileSync(path.join(projectRoot, "src/layers.css"), "utf8")).toContain(
      "@layer reset, theme, base, astryx-base, astryx-theme, product, components, utilities;",
    )
    expect(readFileSync(path.join(projectRoot, "src/ui/foundations/index.ts"), "utf8")).not.toContain("tokens.css")
  })

  test("builds a static CSS artifact with Tailwind and Astryx", async () => {
    const fixtureRoot = mkdtempSync(path.join(os.tmpdir(), "kanban-ui-css-fixture-"))
    const outputRoot = mkdtempSync(path.join(os.tmpdir(), "kanban-ui-css-build-"))
    try {
      const sharedStyleEntry = JSON.stringify(path.join(projectRoot, "src/styles.css"))
      writeFileSync(path.join(fixtureRoot, "main.ts"), `import ${sharedStyleEntry}; document.body.innerHTML = '<div class="flex text-primary">Astryx</div>'`)
      await viteBuild({
        root: fixtureRoot,
        configFile: false,
        plugins: [tailwindcss()],
        build: {
          outDir: outputRoot,
          emptyOutDir: true,
          rollupOptions: { input: path.join(fixtureRoot, "main.ts") },
        },
      })
      const assets = collectFiles(outputRoot)
      const css = assets.filter((file) => file.endsWith(".css")).map((file) => readFileSync(file, "utf8")).join("\n")
      const javascript = assets.filter((file) => file.endsWith(".js")).map((file) => readFileSync(file, "utf8")).join("\n")
      expect(css.length).toBeGreaterThan(0)
      expect(css).toMatch(/\.flex\b|\.text-primary\b|\.bg-surface\b/)
      expect(css).toMatch(/\.astryx-button\b|\.astryx-card\b|--color-text-primary/)
      expect(css).not.toContain("<style")
      expect(css).not.toContain("runtime style injection")
      expect(javascript).not.toMatch(/createElement\(["']style["']\)|\.insertRule\(|style\.textContent/)
    } finally {
      rmSync(fixtureRoot, { recursive: true, force: true })
      rmSync(outputRoot, { recursive: true, force: true })
    }
  })
})

function collectFiles(root: string): string[] {
  const files: string[] = []
  for (const entry of readdirSync(root, { withFileTypes: true })) {
    const absolutePath = path.join(root, entry.name)
    if (entry.isDirectory()) files.push(...collectFiles(absolutePath))
    else if (entry.isFile()) files.push(absolutePath)
  }
  return files
}
