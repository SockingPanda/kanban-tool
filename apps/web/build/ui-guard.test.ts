import { mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, symlinkSync, writeFileSync } from "node:fs"
import os from "node:os"
import path from "node:path"

import tailwindcss from "@tailwindcss/vite"
import { build as viteBuild } from "vite"
import { describe, expect, test } from "vitest"

import {
  readUiGuardManifest,
  scanUiSource,
  uiGuardModeFromEnv,
  validateUiGuardManifest,
  validateUiGuardManifestProvenance,
  type UiGuardManifest,
} from "./ui-guard"

const projectRoot = path.resolve(import.meta.dirname, "..")

describe("Astryx CSP UI guard", () => {
  test("loads the machine-readable manifest and shared page contract", () => {
    const manifest = readUiGuardManifest()
    expect(manifest.schemaVersion).toBe(1)
    expect(manifest.rootBarrel).toEqual({ path: "src/ui/astryx/index.ts", mode: "explicit" })
    expect(manifest.safeWrappers.find((entry) => entry.kind === "group")).toMatchObject({ origin: "astryx-facade", component: "SafeCoreFacades" })
    expect(manifest.swizzles).toEqual([])
    expect(manifest.tailwindBridge.version).toBe("4.3.3")
    expect(manifest.cssImports).toContainEqual({ importer: "src/main.tsx", path: "src/styles.css" })
    expect(manifest.pageContract.sharedStyleEntry).toBe("src/styles.css")
    expect(manifest.pageContract.entrypoints).toEqual(expect.arrayContaining([
      { path: "src/main.tsx", import: "./styles.css" },
      { path: ".storybook/preview.tsx", import: "../src/styles.css" },
    ]))
    expect(() => validateUiGuardManifest(manifest)).not.toThrow()
  })

  test("enforces the origin discriminant and evidence requirements", () => {
    const appOwned = cloneManifest()
    const appOwnedEntry = componentEntry(appOwned, "TextArea") as Record<string, unknown>
    appOwnedEntry.origin = "astryx-facade"
    expect(() => validateUiGuardManifest(appOwned)).toThrow()

    const reimplementation = cloneManifest()
    const reimplementationEntry = componentEntry(reimplementation, "TextInput") as Record<string, unknown>
    delete reimplementationEntry.cli
    expect(() => validateUiGuardManifest(reimplementation)).toThrow()

    const swizzle = cloneManifest()
    const swizzleEntry = componentEntry(swizzle, "TextInput") as Record<string, unknown>
    swizzleEntry.origin = "astryx-swizzle"
    expect(() => validateUiGuardManifest(swizzle)).toThrow()

    const facade = cloneManifest()
    const facadeGroup = facade.safeWrappers.find((entry) => entry.kind === "group")
    expect(facadeGroup?.origin).toBe("astryx-facade")
    expect(() => validateUiGuardManifest(facade)).not.toThrow()
    if (facadeGroup?.kind === "group") {
      const textInput = componentEntry(cloneManifest(), "TextInput") as Record<string, unknown>
      facadeGroup.members[0].cli = { ...(textInput.cli as Record<string, unknown>), component: "SafeCard", command: "pnpm exec astryx --json component SafeCard" }
      expect(() => validateUiGuardManifest(facade)).toThrow(/safeWrappers are invalid/)
    }

    const missingStdout = cloneManifest()
    delete (componentEntry(missingStdout, "TextInput").cli as Record<string, unknown>).evidenceStdout
    expect(() => validateUiGuardManifest(missingStdout)).toThrow(/safeWrappers are invalid/)
  })

  test("checks package, lock, export, license, version, source hash, and path provenance", () => {
    const cases: Array<[string, (manifest: UiGuardManifest) => void, RegExp]> = [
      ["source hash", (manifest) => { (groupMember(manifest, "SafeCard").upstream as Record<string, unknown>).sourceSha256 = "0".repeat(64) }, /source hash mismatch/],
      ["package", (manifest) => { (groupMember(manifest, "SafeCard").upstream as Record<string, unknown>).package = "@other/core" }, /upstream metadata/],
      ["version", (manifest) => { (groupMember(manifest, "SafeCard").upstream as Record<string, unknown>).version = "9.9.9" }, /upstream metadata/],
      ["license", (manifest) => { (groupMember(manifest, "SafeCard").upstream as Record<string, unknown>).license = "Apache-2.0" }, /upstream metadata/],
      ["source export", (manifest) => { (groupMember(manifest, "SafeCard").upstream as Record<string, unknown>).sourcePath = "src/HStack/index.ts" }, /package.json exports/],
      ["CLI evidence hash", (manifest) => { (componentEntry(manifest, "TextInput").cli as Record<string, unknown>).evidenceStdout += "forged" }, /stdout hash mismatch/],
      ["lock", (manifest) => { manifest.source.tarballOrSourceHash = `sha512-${"A".repeat(86)}` }, /source hash does not match pnpm-lock/],
    ]
    for (const [, mutate, expected] of cases) {
      const manifest = cloneManifest()
      mutate(manifest)
      expect(() => validateUiGuardManifestProvenance(manifest, projectRoot)).toThrow(expected)
    }

    const traversal = cloneManifest()
    traversal.rootBarrel.path = "src/ui/astryx/../index.ts"
    expect(() => validateUiGuardManifest(traversal)).toThrow(/Invalid Astryx UI safety manifest path/)

    const forgedSource = cloneManifest()
    const selector = componentEntry(forgedSource, "Selector")
    const sharedType = (selector.publicSources as Array<Record<string, unknown>>).find((entry) => entry.exported === "SelectableOption")
    if (sharedType === undefined) throw new Error("missing Selector shared public source fixture")
    sharedType.sourcePath = selector.ownedPath
    expect(() => validateUiGuardManifestProvenance(forgedSource, projectRoot)).toThrow(/export modules/)
  })

  test("rejects symlink escapes and duplicate wrapper identities", () => {
    const temporaryRoot = mkdtempSync(path.join(projectRoot, ".ui-guard-manifest-test-"))
    try {
      symlinkSync("/etc/passwd", path.join(temporaryRoot, "escape.ts"))
      const symlinkManifest = cloneManifest()
      symlinkManifest.safeWrappers[0].ownedPath = `${path.basename(temporaryRoot)}/escape.ts`
      expect(() => validateUiGuardManifestProvenance(symlinkManifest, projectRoot)).toThrow(/regular file/)
    } finally {
      rmSync(temporaryRoot, { recursive: true, force: true })
    }

    const duplicate = cloneManifest()
    duplicate.safeWrappers[1].publicApi = [...duplicate.safeWrappers[1].publicApi, duplicate.safeWrappers[0].publicApi[0]]
    expect(() => validateUiGuardManifestProvenance(duplicate, projectRoot)).toThrow(/duplicate publicApi/)
  })

  test("requires an explicit root barrel with exact runtime and type exports", () => {
    const source = readFileSync(path.join(projectRoot, "src/ui/astryx/index.ts"), "utf8")
    const cases: Array<[string, string, RegExp]> = [
      ["wildcard", "export * from \"./fields/TextInput\"", /wildcard/],
      ["missing", source.replace('export {CheckboxInput} from "./fields/CheckboxInput"\n', ""), /exactly match/],
      ["extra", `${source}\nexport const Extra = 1\n`, /only allows explicit/],
      ["runtime", source.replace('export {CheckboxInput} from "./fields/CheckboxInput"', 'export type {CheckboxInput} from "./fields/CheckboxInput"'), /runtime exports must exactly match/],
      ["type-as-runtime", source.replace('export type {\n  CheckboxInputProps,', 'export {\n  CheckboxInputProps,'), /runtime exports must exactly match/],
      ["duplicate", `${source}\nexport {CheckboxInput} from "./fields/CheckboxInput"\n`, /duplicate export/],
      ["module ownership", source.replace('export {CheckboxInput} from "./fields/CheckboxInput"', 'export {CheckboxInput} from "./fields/TextInput"'), /export modules/],
      ["imported alias", source.replace('export {CheckboxInput} from "./fields/CheckboxInput"', 'export {TextInput as CheckboxInput} from "./fields/TextInput"'), /export modules/],
    ]
    for (const [, barrel, expected] of cases) {
      const temporaryBarrel = path.join(projectRoot, "src/ui/astryx", `.ui-guard-root-${process.pid}-${Date.now()}-${Math.random().toString(16).slice(2)}.ts`)
      try {
        writeFileSync(temporaryBarrel, barrel)
        const manifest = cloneManifest()
        manifest.rootBarrel.path = `src/ui/astryx/${path.basename(temporaryBarrel)}`
        expect(() => validateUiGuardManifestProvenance(manifest, projectRoot)).toThrow(expected)
      } finally {
        rmSync(temporaryBarrel, { force: true })
      }
    }
  })

  test("keeps safe-wrapper provenance independent from unsafe direct-import severity", () => {
    const temporaryRoot = mkdtempSync(path.join(os.tmpdir(), "kanban-ui-guard-safe-wrapper-"))
    try {
      writeFileSync(path.join(temporaryRoot, "unsafe.tsx"), 'import "@astryxdesign/core/TextInput"')
      const manifest = cloneManifest()
      const textInput = componentEntry(manifest, "TextInput") as Record<string, unknown>
      textInput.origin = "app-owned"
      delete textInput.upstream
      delete textInput.cli
      const report = scanUiSource({ projectRoot: temporaryRoot, sourcePaths: ["unsafe.tsx"], mode: "enforce", manifest })
      expect(report.errors.some((error) => error.code === "unsafe-direct-import")).toBe(true)
    } finally {
      rmSync(temporaryRoot, { recursive: true, force: true })
    }
  })

  test("requires swizzle safe wrappers to match legacy swizzle provenance", () => {
    const manifest = cloneManifest()
    const textInput = componentEntry(manifest, "TextInput")
    textInput.origin = "astryx-swizzle"
    const cli = textInput.cli as Record<string, unknown>
    cli.command = "pnpm exec astryx --json swizzle TextInput"
    manifest.ownedPaths = [...manifest.ownedPaths, textInput.ownedPath]
    expect(() => validateUiGuardManifestProvenance(manifest, projectRoot)).toThrow(/missing from legacy/)
    manifest.swizzles = [{
      component: "TextInput",
      ownedPath: textInput.ownedPath,
      sourcePackage: "@astryxdesign/core",
      sourceVersion: "0.3.0",
      sourcePath: "src/TextInput/index.ts",
      command: "pnpm exec astryx --json swizzle TextInput",
      sourceSha256: textInput.upstream.sourceSha256,
      reason: "test",
      publicApi: "TextInput",
      license: "MIT",
      runtimeStylePolicy: "static",
    }]
    expect(() => validateUiGuardManifestProvenance(manifest, projectRoot)).not.toThrow()
    manifest.swizzles[0].publicApi = "Other"
    expect(() => validateUiGuardManifestProvenance(manifest, projectRoot)).toThrow(/matching safeWrapper/)
    manifest.swizzles[0].publicApi = "TextInput"
    manifest.swizzles[0].sourcePackage = "@other/core"
    expect(() => validateUiGuardManifestProvenance(manifest, projectRoot)).toThrow(/provenance does not match/)
  })

  test("inventories the current source without traversing reference/output", () => {
    const report = scanUiSource({ projectRoot, sourcePaths: ["src", ".storybook", "vite.config.ts"] })
    expect(report.passed).toBe(true)
    expect(report.files.some((file) => file.includes(`${path.sep}stories${path.sep}reference${path.sep}`))).toBe(false)
    expect(report.warnings.some((warning) => warning.code === "residual-css")).toBe(true)
  }, 60_000)

  test("fails closed for unsafe source imports, inline style, DOM style, and forbidden paths", () => {
    const temporaryRoot = mkdtempSync(path.join(os.tmpdir(), "kanban-ui-guard-"))
    try {
      writeFileSync(path.join(temporaryRoot, "src.tsx"), [
        'import "@astryxdesign/core/TextInput"',
        'import { TextInput as UnsafeInput } from "@astryxdesign/core"',
        'import "./reference/example"',
        'import "./src%2Fstories%2Freference%2Fruntime"',
        'import ".\\\\output\\\\runtime"',
        'import "./reference/%ZZ"',
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
      expect(report.errors.filter((error) => error.code === "forbidden-import")).toHaveLength(4)
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
        'const Alias = Core; const loaderAlias = dynamicCore',
        'void Core.Grid; void Core["TextInput"]; void core.Tooltip; void Alias.Grid; void loaderAlias.Dialog',
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

  test("does not trust out-of-scope or mutable object spread bindings", () => {
    const temporaryRoot = mkdtempSync(path.join(os.tmpdir(), "kanban-ui-guard-scope-"))
    try {
      writeFileSync(path.join(temporaryRoot, "scope.tsx"), [
        'const safe = { className: "flex" }',
        'const postUse = <div {...postProps} />; const postProps = { className: "flex" }',
        'function Shadowed() { const safe = { style: { color: "red" } }; return <div {...safe} /> }',
        'let reassigned = { className: "flex" }; reassigned = { style: { color: "red" } }; const Reassigned = () => <div {...reassigned} />',
        'const mutated = { className: "flex" }; mutated.style = { color: "red" }; const Mutated = () => <div {...mutated} />',
        'const polluted = { className: "flex" }; Object.assign(polluted, { style: { color: "red" } }); const Polluted = () => <div {...polluted} />',
        'declare function sink(value: unknown): void; const escaped = { className: "flex" }; sink(escaped); const Escaped = () => <div {...escaped} />',
        'const Safe = () => <div {...safe} />',
      ].join("\n"))
      const report = scanUiSource({ projectRoot: temporaryRoot, sourcePaths: ["scope.tsx"], mode: "enforce" })
      const spreadErrors = report.errors.filter((error) => error.path === "scope.tsx" && error.code === "inline-style")
      expect(spreadErrors).toHaveLength(6)
    } finally {
      rmSync(temporaryRoot, { recursive: true, force: true })
    }
  })

  test("only trusts inline calls from the canonical DOM-prop helper", () => {
    const temporaryRoot = mkdtempSync(path.join(os.tmpdir(), "kanban-ui-guard-dom-props-"))
    try {
      const sourceDirectory = path.join(temporaryRoot, "src/ui/astryx/fields")
      mkdirSync(sourceDirectory, {recursive: true})
      writeFileSync(path.join(sourceDirectory, "safe.tsx"), [
        'import {pickCspSafeDomProps} from "../dom-props"',
        'declare const props: Record<string, unknown>',
        'const Direct = () => <input {...pickCspSafeDomProps({style: {color: "red"}})} />',
      ].join("\n"))
      writeFileSync(path.join(sourceDirectory, "unsafe-results.tsx"), [
        'import {pickCspSafeDomProps} from "../dom-props"',
        'declare const props: Record<string, unknown>',
        'const safe = pickCspSafeDomProps(props)',
        'safe.style = {color: "red"}',
        'Object.assign(safe, {style: {color: "red"}})',
        'declare function sink(value: unknown): void; sink(safe)',
        'const alias = safe',
        'const Mutated = () => <input {...safe} />',
        'const Polluted = () => <input {...safe} />',
        'const Escaped = () => <input {...safe} />',
        'const Aliased = () => <input {...alias} />',
      ].join("\n"))
      writeFileSync(path.join(sourceDirectory, "same-name.tsx"), [
        'const pickCspSafeDomProps = (props: Record<string, unknown>) => props',
        'const Bad = () => <input {...pickCspSafeDomProps({style: {color: "red"}})} />',
      ].join("\n"))
      writeFileSync(path.join(sourceDirectory, "wrong-module.tsx"), [
        'import {pickCspSafeDomProps} from "./other-dom-props"',
        'const Bad = () => <input {...pickCspSafeDomProps({style: {color: "red"}})} />',
      ].join("\n"))
      writeFileSync(path.join(sourceDirectory, "same-class-name.tsx"), [
        'import {pickCspSafeDomProps} from "../dom-props"',
        'class pickCspSafeDomProps {}',
        'const Bad = () => <input {...pickCspSafeDomProps({style: {color: "red"}})} />',
      ].join("\n"))
      writeFileSync(path.join(sourceDirectory, "expression-shadows.tsx"), [
        'import {pickCspSafeDomProps, pickCspSafeDomProps as sanitize} from "../dom-props"',
        'const FunctionShadow = function pickCspSafeDomProps(props: Record<string, unknown>) { return <input {...pickCspSafeDomProps({style: {color: "red"}})} /> }',
        'const AliasFunctionShadow = function sanitize(props: Record<string, unknown>) { return <input {...sanitize({style: {color: "red"}})} /> }',
        'const ClassShadow = class pickCspSafeDomProps { render() { return <input {...pickCspSafeDomProps({style: {color: "red"}})} /> } }',
        'const AliasClassShadow = class sanitize { render() { return <input {...sanitize({style: {color: "red"}})} /> } }',
        'function VarShadow() { if (true) { var pickCspSafeDomProps = (props: Record<string, unknown>) => props }; return <input {...pickCspSafeDomProps({style: {color: "red"}})} /> }',
        'function VarAliasShadow() { if (true) { var sanitize = (props: Record<string, unknown>) => props }; return <input {...sanitize({style: {color: "red"}})} /> }',
      ].join("\n"))

      const report = scanUiSource({
        projectRoot: temporaryRoot,
        sourcePaths: ["src/ui/astryx/fields"],
        mode: "enforce",
      })
      expect(report.errors.filter((error) => error.path === "src/ui/astryx/fields/safe.tsx" && error.code === "inline-style")).toHaveLength(0)
      expect(report.errors.filter((error) => error.path === "src/ui/astryx/fields/unsafe-results.tsx" && error.code === "inline-style")).toHaveLength(4)
      expect(report.errors.filter((error) => error.path === "src/ui/astryx/fields/same-name.tsx" && error.code === "inline-style")).toHaveLength(1)
      expect(report.errors.filter((error) => error.path === "src/ui/astryx/fields/wrong-module.tsx" && error.code === "inline-style")).toHaveLength(1)
      expect(report.errors.filter((error) => error.path === "src/ui/astryx/fields/same-class-name.tsx" && error.code === "inline-style")).toHaveLength(1)
      expect(report.errors.filter((error) => error.path === "src/ui/astryx/fields/expression-shadows.tsx" && error.code === "inline-style")).toHaveLength(6)
    } finally {
      rmSync(temporaryRoot, {recursive: true, force: true})
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
  }, 60_000)

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
  }, 60_000)
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

function cloneManifest(): UiGuardManifest {
  return JSON.parse(JSON.stringify(readUiGuardManifest())) as UiGuardManifest
}

function componentEntry(manifest: UiGuardManifest, component: string): Record<string, unknown> {
  const entry = manifest.safeWrappers.find((candidate) => candidate.kind === "component" && candidate.component === component)
  if (entry === undefined || entry.kind !== "component") throw new Error(`missing test wrapper: ${component}`)
  return entry as unknown as Record<string, unknown>
}

function groupMember(manifest: UiGuardManifest, component: string): Record<string, unknown> {
  for (const entry of manifest.safeWrappers) {
    if (entry.kind !== "group") continue
    const member = entry.members.find((candidate) => candidate.component === component)
    if (member !== undefined) return member as unknown as Record<string, unknown>
  }
  throw new Error(`missing test group member: ${component}`)
}
