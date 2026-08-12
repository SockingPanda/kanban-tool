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
    expect(manifest.cssImports).toContainEqual({ importer: "src/main.tsx", path: "src/styles.css" })
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

  test("catches barrel members, re-exports, dynamic loaders, and unsafe style sinks", () => {
    const temporaryRoot = mkdtempSync(path.join(os.tmpdir(), "kanban-ui-guard-hardening-"))
    try {
      writeFileSync(path.join(temporaryRoot, "unsafe.tsx"), [
        'import * as Core from "@astryxdesign/core"',
        'export { Grid } from "@astryxdesign/core"',
        'export * from "@astryxdesign/core"',
        'export * as UnsafeCore from "@astryxdesign/core"',
        'const core = require("@astryxdesign/core")',
        'const dynamicCore = await import("@astryxdesign/core")',
        'void Core.Grid; void Core["TextInput"]; void core.Tooltip',
        'void dynamicCore.Dialog',
        'void import("@astryxdesign/core/Popover")',
        'void import("./reference/runtime")',
        'void require("./output/runtime")',
        'const styleProps = { style: { color: "red" } }',
        'const Bad = () => <div {...{ style: { color: "red" } }} {...styleProps} dangerouslySetInnerHTML={{ __html: "<style>" }} />',
        'React.createElement("div", { style: { color: "red" } })',
        'document.body.setAttribute("style", "color:red")',
        'document.body.removeAttribute("style")',
        'document.body.insertAdjacentHTML("beforeend", "<style>")',
        'document.body.innerHTML = "<style>"',
      ].join("\n"))
      const report = scanUiSource({ projectRoot: temporaryRoot, sourcePaths: ["unsafe.tsx"], mode: "enforce" })
      expect(report.passed).toBe(false)
      expect(report.errors.filter((error) => error.code === "unsafe-direct-import").length).toBeGreaterThanOrEqual(6)
      expect(report.errors.filter((error) => error.code === "forbidden-import")).toHaveLength(2)
      expect(report.errors.some((error) => error.code === "inline-style")).toBe(true)
      expect(report.errors.some((error) => error.code === "dom-style")).toBe(true)
    } finally {
      rmSync(temporaryRoot, { recursive: true, force: true })
    }
  })

  test("blocks CSS edges outside the importer/path baseline", () => {
    const temporaryRoot = mkdtempSync(path.join(os.tmpdir(), "kanban-ui-css-baseline-"))
    try {
      writeFileSync(path.join(temporaryRoot, "new.tsx"), 'import "./new-feature.css"')
      const inventory = scanUiSource({ projectRoot: temporaryRoot, sourcePaths: ["new.tsx"] })
      expect(inventory.errors.some((error) => error.code === "css-import")).toBe(false)
      expect(inventory.warnings.some((warning) => warning.code === "css-import")).toBe(true)
      const enforce = scanUiSource({ projectRoot: temporaryRoot, sourcePaths: ["new.tsx"], mode: "enforce" })
      expect(enforce.errors.some((error) => error.code === "css-import")).toBe(true)
    } finally {
      rmSync(temporaryRoot, { recursive: true, force: true })
    }
  })

  test("fails closed for dynamic module names and unknown intrinsic style bags", () => {
    const temporaryRoot = mkdtempSync(path.join(os.tmpdir(), "kanban-ui-guard-v2-"))
    try {
      writeFileSync(path.join(temporaryRoot, "unsafe.tsx"), [
        'declare const suffix: string, modulePath: string, props: Record<string, unknown>, attr: string',
        'void import(`./feature/${suffix}`)',
        'void import("./feature/" + suffix)',
        'void require(modulePath)',
        'void require(`./feature/${suffix}`)',
        'const styled = { style: { color: "red" } }',
        'const dangerous = { dangerouslySetInnerHTML: { __html: "<style>" } }',
        'const Bad = () => <div {...props} {...styled} {...dangerous} />',
        'React.createElement("div", props)',
        'React.createElement("div", styled)',
        'document.body.setAttribute(attr, "red")',
        'Object.assign(document.body, { style: { color: "red" } })',
        'Object.assign(document.body, props)',
      ].join("\n"))
      writeFileSync(path.join(temporaryRoot, "safe.tsx"), [
        'const safe = { className: "flex", ariaLabel: "safe" }',
        'const Safe = () => <div {...safe} />',
        'React.createElement("div", safe)',
        'document.body.setAttribute("data-state", "ready")',
        'Object.assign({}, { body: "safe" })',
      ].join("\n"))
      const report = scanUiSource({ projectRoot: temporaryRoot, sourcePaths: ["unsafe.tsx", "safe.tsx"], mode: "enforce" })
      expect(report.passed).toBe(false)
      expect(report.errors.filter((error) => error.code === "unsafe-direct-import").some((error) => error.message.includes("module specifier"))).toBe(true)
      expect(report.errors.filter((error) => error.code === "inline-style").length).toBeGreaterThanOrEqual(4)
      expect(report.errors.filter((error) => error.code === "dom-style").length).toBeGreaterThanOrEqual(3)
      expect(report.errors.filter((error) => error.path === "safe.tsx" && ["unsafe-direct-import", "inline-style", "dom-style"].includes(error.code))).toHaveLength(0)
    } finally {
      rmSync(temporaryRoot, { recursive: true, force: true })
    }
  })

  test("resolves only the supported inventory and enforce modes", () => {
    expect(uiGuardModeFromEnv("inventory")).toBe("inventory")
    expect(uiGuardModeFromEnv("enforce")).toBe("enforce")
    expect(() => uiGuardModeFromEnv("strict")).toThrow(/inventory or enforce/)
    const enforce = scanUiSource({ projectRoot, sourcePaths: ["src/ui/foundations/tokens.css"], mode: "enforce" })
    expect(enforce.errors.some((error) => error.code === "residual-css")).toBe(false)
    expect(enforce.warnings.some((warning) => warning.code === "residual-css")).toBe(true)
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

  test("keeps production and Storybook CSS topology identical and same-origin", async () => {
    const fixtureRoot = mkdtempSync(path.join(os.tmpdir(), "kanban-ui-config-fixture-"))
    const productionOutput = mkdtempSync(path.join(os.tmpdir(), "kanban-ui-config-production-"))
    const storybookOutput = mkdtempSync(path.join(os.tmpdir(), "kanban-ui-config-storybook-"))
    try {
      writeFileSync(path.join(fixtureRoot, "index.html"), '<div id="root"></div><script type="module" src="/main.ts"></script>')
      writeFileSync(path.join(fixtureRoot, "main.ts"), `import ${JSON.stringify(path.join(projectRoot, "src/styles.css"))}; document.getElementById("root")?.classList.add("flex", "text-primary")`)
      await viteBuild({
        root: fixtureRoot,
        configFile: path.join(projectRoot, "vite.config.ts"),
        build: { outDir: productionOutput, emptyOutDir: true, rollupOptions: { input: path.join(fixtureRoot, "index.html") } },
      })
      await viteBuild({
        root: fixtureRoot,
        configFile: path.join(projectRoot, ".storybook/vite.config.ts"),
        build: { outDir: storybookOutput, emptyOutDir: true, rollupOptions: { input: path.join(fixtureRoot, "index.html") } },
      })

      const production = readBuiltCssTopology(productionOutput)
      const storybook = readBuiltCssTopology(storybookOutput)
      expect(production.cssFiles).toHaveLength(1)
      expect(storybook.cssFiles).toHaveLength(1)
      expect(production.firstLayer).toBe(storybook.firstLayer)
      expect(production.firstLayer).toMatch(/^@layer\s+[\w-]+/)
      expect(production.html).toMatch(/href=["'][^"']+\.css["']/)
      expect(storybook.html).toMatch(/href=["'][^"']+\.css["']/)
      expect(production.html).not.toMatch(/href=["'][a-z][a-z0-9+.-]*:\/\//i)
      expect(storybook.html).not.toMatch(/href=["'][a-z][a-z0-9+.-]*:\/\//i)
      expect(production.css).toContain("--color-text-primary")
      expect(storybook.css).toContain("--color-text-primary")
    } finally {
      rmSync(fixtureRoot, { recursive: true, force: true })
      rmSync(productionOutput, { recursive: true, force: true })
      rmSync(storybookOutput, { recursive: true, force: true })
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

function readBuiltCssTopology(root: string): { cssFiles: string[]; css: string; html: string; firstLayer: string } {
  const files = collectFiles(root)
  const cssFiles = files.filter((file) => file.endsWith(".css"))
  const css = cssFiles.map((file) => readFileSync(file, "utf8")).join("\n")
  const html = files.filter((file) => file.endsWith(".html")).map((file) => readFileSync(file, "utf8")).join("\n")
  const firstLayer = css.match(/@layer\s+[\w-]+/)?.[0] ?? ""
  return { cssFiles, css, html, firstLayer }
}
