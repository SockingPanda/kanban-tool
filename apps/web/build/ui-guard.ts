import { existsSync, lstatSync, readFileSync, readdirSync, realpathSync } from "node:fs"
import path from "node:path"
import { fileURLToPath } from "node:url"

import type { Plugin } from "vite"
import ts from "typescript"

export const UI_GUARD_SCHEMA_VERSION = 1
export const UI_GUARD_MANIFEST_PATH = fileURLToPath(new URL("../astryx-safe.manifest.json", import.meta.url))
export const UI_GUARD_DEFAULT_MODE: UiGuardMode = "inventory"

export type UiGuardMode = "inventory" | "enforce"

/** Resolve the migration mode from the environment without accepting typos. */
export function uiGuardModeFromEnv(value: unknown = process.env.UI_GUARD_MODE): UiGuardMode {
  if (value === undefined || value === "" || value === "inventory") return UI_GUARD_DEFAULT_MODE
  if (value === "enforce") return "enforce"
  throw new Error(`Invalid UI_GUARD_MODE: ${String(value)}; expected inventory or enforce`)
}

export type UiGuardSource = {
  readonly package: string
  readonly version: string
  readonly tarballOrSourceHash: string
  readonly license: string
  readonly upstream: string
}

export type UiGuardTailwindBridge = {
  readonly packages: readonly string[]
  readonly version: string
  readonly staticCssEntry: string
}

export type UiGuardSwizzle = {
  readonly component: string
  readonly ownedPath: string
  readonly sourcePackage: string
  readonly sourceVersion: string
  readonly sourcePath: string
  readonly command: string
  readonly sourceSha256: string
  readonly reason: string
  readonly publicApi: string
  readonly license: string
  readonly runtimeStylePolicy: string
}

export type UiGuardResidualCss = {
  readonly path: string
  readonly maxLoc: number
  readonly owner: string
  readonly reason: string
  readonly exitCondition: string
}

export type UiGuardLegacyRawLayout = {
  readonly path: string
  readonly maxCount: number
  readonly owner: string
  readonly exitCondition: string
}

export type UiGuardCssImport = {
  readonly importer: string
  readonly path: string
}

export type UiGuardUnsafeImportBaseline = {
  readonly importer: string
  readonly path: string
}

export type UiGuardPageEntrypoint = {
  readonly path: string
  readonly import: string
}

export type UiGuardPageContract = {
  readonly schemaVersion: number
  readonly sharedStyleEntry: string
  readonly entrypoints: readonly UiGuardPageEntrypoint[]
}

export type UiGuardManifest = {
  readonly schemaVersion: number
  readonly source: UiGuardSource
  readonly tailwindBridge: UiGuardTailwindBridge
  readonly swizzles: readonly UiGuardSwizzle[]
  readonly unsafeDirectImports: readonly string[]
  readonly unsafeImportBaseline: readonly UiGuardUnsafeImportBaseline[]
  readonly ownedPaths: readonly string[]
  readonly forbiddenRoots: readonly string[]
  readonly residualCss: readonly UiGuardResidualCss[]
  readonly legacyRawLayout: readonly UiGuardLegacyRawLayout[]
  readonly cssImports: readonly UiGuardCssImport[]
  readonly pageContract: UiGuardPageContract
}

export type UiGuardIssueCode =
  | "manifest-invalid"
  | "forbidden-path"
  | "forbidden-import"
  | "unsafe-direct-import"
  | "inline-style"
  | "dom-style"
  | "unowned-css"
  | "residual-css"
  | "residual-css-over-budget"
  | "legacy-raw-layout"
  | "legacy-raw-layout-over-budget"
  | "css-import"
  | "manifest-provenance"
  | "page-contract"

export type UiGuardIssue = {
  readonly code: UiGuardIssueCode
  readonly severity: "warning" | "error"
  readonly path?: string
  readonly line?: number
  readonly message: string
}

export type UiGuardReport = {
  readonly mode: UiGuardMode
  readonly files: readonly string[]
  readonly issues: readonly UiGuardIssue[]
  readonly errors: readonly UiGuardIssue[]
  readonly warnings: readonly UiGuardIssue[]
  readonly passed: boolean
}

export type UiGuardOptions = {
  readonly projectRoot?: string
  readonly manifest?: UiGuardManifest
  readonly mode?: UiGuardMode
  readonly sourcePaths?: readonly string[]
}

export class UiGuardError extends Error {
  readonly report: UiGuardReport

  constructor(report: UiGuardReport) {
    super(formatUiGuardIssues(report))
    this.name = "UiGuardError"
    this.report = report
  }
}

/** Read and validate the checked-in machine-readable UI safety manifest. */
export function readUiGuardManifest(manifestPath = UI_GUARD_MANIFEST_PATH): UiGuardManifest {
  let parsed: unknown
  try {
    parsed = JSON.parse(readFileSync(manifestPath, "utf8")) as unknown
  } catch (error) {
    throw new Error(`Unable to read Astryx UI safety manifest: ${manifestPath}`, { cause: error })
  }
  validateUiGuardManifest(parsed)
  validateUiGuardManifestProvenance(parsed, path.resolve(path.dirname(manifestPath)))
  return parsed
}

/** Validate the manifest shape before it can influence a guard decision. */
export function validateUiGuardManifest(value: unknown): asserts value is UiGuardManifest {
  if (!isRecord(value)) throw new Error("Astryx UI safety manifest must be an object")
  if (value.schemaVersion !== UI_GUARD_SCHEMA_VERSION) {
    throw new Error(`Unsupported Astryx UI safety manifest schemaVersion: ${String(value.schemaVersion)}`)
  }
  const source = value.source
  if (!isRecord(source)) throw new Error("Astryx UI safety manifest source must be an object")
  for (const key of ["package", "version", "tarballOrSourceHash", "license", "upstream"] as const) {
    if (!isNonEmptyString(source[key])) throw new Error(`Astryx UI safety manifest source.${key} must be a string`)
  }

  const bridge = value.tailwindBridge
  if (!isRecord(bridge) || !isStringArray(bridge.packages) || bridge.packages.length === 0 || !isNonEmptyString(bridge.version) || !isNonEmptyString(bridge.staticCssEntry)) {
    throw new Error("Astryx UI safety manifest tailwindBridge is invalid")
  }
  if (!isStringArray(value.unsafeDirectImports) || !isStringArray(value.ownedPaths) || !isStringArray(value.forbiddenRoots)) {
    throw new Error("Astryx UI safety manifest path lists are invalid")
  }
  if (!Array.isArray(value.unsafeImportBaseline) || !value.unsafeImportBaseline.every(isUnsafeImportBaseline)) {
    throw new Error("Astryx UI safety manifest unsafeImportBaseline is invalid")
  }
  if (!Array.isArray(value.swizzles) || !value.swizzles.every(isSwizzle)) {
    throw new Error("Astryx UI safety manifest swizzles are invalid")
  }
  if (!Array.isArray(value.residualCss) || !value.residualCss.every(isResidualCss)) {
    throw new Error("Astryx UI safety manifest residualCss is invalid")
  }
  if (!Array.isArray(value.legacyRawLayout) || !value.legacyRawLayout.every(isLegacyRawLayout)) {
    throw new Error("Astryx UI safety manifest legacyRawLayout is invalid")
  }
  if (!Array.isArray(value.cssImports) || !value.cssImports.every(isCssImport)) {
    throw new Error("Astryx UI safety manifest cssImports is invalid")
  }
  if (!isRecord(value.pageContract) || value.pageContract.schemaVersion !== UI_GUARD_SCHEMA_VERSION || !isNonEmptyString(value.pageContract.sharedStyleEntry) || !Array.isArray(value.pageContract.entrypoints) || !value.pageContract.entrypoints.every(isPageEntrypoint)) {
    throw new Error("Astryx UI safety manifest pageContract is invalid")
  }
  for (const candidate of [...value.ownedPaths, ...value.forbiddenRoots, ...value.residualCss.map((item) => item.path), ...value.legacyRawLayout.map((item) => item.path), value.pageContract.sharedStyleEntry, ...value.pageContract.entrypoints.map((item) => item.path), ...value.cssImports.map((item) => item.importer), ...value.unsafeImportBaseline.map((item) => item.importer)]) {
    validateManifestPath(candidate)
  }
  for (const entry of value.cssImports) validateCssImportPath(entry.path)
  for (const entry of value.unsafeImportBaseline) {
    if (entry.path.startsWith("/") || entry.path.includes("\\") || entry.path.split("/").includes("..")) {
      throw new Error(`Invalid Astryx UI safety manifest unsafe import path: ${entry.path}`)
    }
  }
}

/** Validate provenance against the checked-in package metadata and lockfile before scanning sources. */
function validateUiGuardManifestProvenance(manifest: UiGuardManifest, projectRoot: string): void {
  const packageJsonPath = path.join(projectRoot, "package.json")
  const lockfilePath = path.resolve(projectRoot, "..", "..", "pnpm-lock.yaml")
  let packageJson: { dependencies?: Record<string, unknown>; devDependencies?: Record<string, unknown> }
  try {
    packageJson = JSON.parse(readFileSync(packageJsonPath, "utf8")) as typeof packageJson
  } catch (error) {
    throw new Error(`Unable to validate Astryx manifest provenance: ${packageJsonPath}`, { cause: error })
  }
  const corePackagePath = path.join(projectRoot, "node_modules", "@astryxdesign", "core", "package.json")
  let corePackage: { version?: unknown; license?: unknown }
  try {
    corePackage = JSON.parse(readFileSync(corePackagePath, "utf8")) as typeof corePackage
  } catch (error) {
    throw new Error(`Unable to validate Astryx package provenance: ${corePackagePath}`, { cause: error })
  }
  if (corePackage.version !== manifest.source.version || corePackage.license !== manifest.source.license) {
    throw new Error(`Astryx manifest source metadata does not match installed ${manifest.source.package}`)
  }
  const packageVersion = packageJson.dependencies?.[manifest.source.package]
  if (packageVersion !== manifest.source.version) {
    throw new Error(`Astryx manifest source version does not match package.json (${manifest.source.package})`)
  }
  for (const packageName of ["tailwindcss", "@tailwindcss/vite"]) {
    const declared = packageJson.dependencies?.[packageName] ?? packageJson.devDependencies?.[packageName]
    if (declared !== manifest.tailwindBridge.version) {
      throw new Error(`Astryx Tailwind bridge version does not match package.json (${packageName})`)
    }
  }
  let lockfile = ""
  try {
    lockfile = readFileSync(lockfilePath, "utf8")
  } catch (error) {
    throw new Error(`Unable to validate Astryx lockfile provenance: ${lockfilePath}`, { cause: error })
  }
  const lockPattern = new RegExp(`['"]?${escapeRegExp(manifest.source.package)}@${escapeRegExp(manifest.source.version)}['"]?:\\n\\s+resolution: \\{integrity: ([^}]+)\\}`)
  const lockMatch = lockfile.match(lockPattern)
  if (lockMatch?.[1] !== `sha512-${manifest.source.tarballOrSourceHash.replace(/^sha512-/, "")}`) {
    throw new Error(`Astryx manifest source hash does not match pnpm-lock.yaml (${manifest.source.package})`)
  }
  const packageIntegrityPattern = /^sha(256|384|512)-[A-Za-z0-9+/]+={0,2}$/
  if (!packageIntegrityPattern.test(manifest.source.tarballOrSourceHash)) {
    throw new Error("Astryx manifest source.tarballOrSourceHash must be a valid SRI hash")
  }
  const owned = new Set<string>()
  const canonicalRoot = realpathSync(projectRoot)
  for (const ownedPath of manifest.ownedPaths) {
    if (owned.has(ownedPath)) throw new Error(`Astryx manifest ownedPaths contains a duplicate: ${ownedPath}`)
    owned.add(ownedPath)
    const absolutePath = path.resolve(projectRoot, ownedPath)
    if (!existsSync(absolutePath) || !isRegularFile(absolutePath)) {
      throw new Error(`Astryx manifest ownedPath must be an existing file under the app root: ${ownedPath}`)
    }
    const canonicalPath = realpathSync(absolutePath)
    if (canonicalPath !== canonicalRoot && !canonicalPath.startsWith(`${canonicalRoot}${path.sep}`)) {
      throw new Error(`Astryx manifest ownedPath must remain under the canonical app root: ${ownedPath}`)
    }
  }
  for (const swizzle of manifest.swizzles) {
    if (!owned.has(swizzle.ownedPath)) {
      throw new Error(`Astryx manifest swizzle ownedPath must be listed in ownedPaths: ${swizzle.component}`)
    }
    if (!/^[a-f0-9]{64}$/i.test(swizzle.sourceSha256)) {
      throw new Error(`Astryx manifest swizzle sourceSha256 must be 64 hexadecimal characters: ${swizzle.component}`)
    }
    if (swizzle.sourceVersion !== manifest.source.version || swizzle.license !== manifest.source.license) {
      throw new Error(`Astryx manifest swizzle provenance does not match ${manifest.source.package}: ${swizzle.component}`)
    }
  }
  for (const entry of manifest.cssImports) {
    if (!isRegularFile(path.join(projectRoot, entry.importer))) {
      throw new Error(`Astryx manifest CSS importer must be an existing file: ${entry.importer}`)
    }
    if (isAppRelativeCssPath(entry.path) && !isRegularFile(path.join(projectRoot, entry.path))) {
      throw new Error(`Astryx manifest CSS path must be an existing app file: ${entry.path}`)
    }
  }
  for (const entry of manifest.unsafeImportBaseline) {
    if (!isRegularFile(path.join(projectRoot, entry.importer))) {
      throw new Error(`Astryx manifest unsafe import baseline importer must be an existing file: ${entry.importer}`)
    }
  }
}

function isAppRelativeCssPath(value: string): boolean {
  return value.startsWith("src/") || value.startsWith(".storybook/")
}

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")
}

/** Scan source paths without traversing the explicitly forbidden reference/output trees. */
export function scanUiSource(options: UiGuardOptions = {}): UiGuardReport {
  const mode = options.mode ?? UI_GUARD_DEFAULT_MODE
  const projectRoot = path.resolve(options.projectRoot ?? path.resolve(import.meta.dirname, ".."))
  const manifest = options.manifest ?? readUiGuardManifest()
  validateUiGuardManifest(manifest)

  const issues: UiGuardIssue[] = []
  const files: string[] = []
  const sourcePaths = options.sourcePaths ?? ["src", ".storybook", "vite.config.ts"]

  for (const sourcePath of sourcePaths) {
    const absolutePath = path.resolve(projectRoot, sourcePath)
    if (isForbiddenPath(absolutePath, projectRoot, manifest.forbiddenRoots)) {
      issues.push(issue("forbidden-path", "error", sourcePath, undefined, "UI guard source roots must not include reference/output paths."))
      continue
    }
    collectSourceFiles(absolutePath, projectRoot, manifest.forbiddenRoots, files)
  }
  files.sort()

  for (const file of files) {
    const relativePath = toManifestPath(path.relative(projectRoot, file))
    const source = readFileSync(file, "utf8")
    scanSourceText(source, relativePath, mode, manifest, issues)
    if (relativePath.endsWith(".css")) scanCssInventory(source, relativePath, mode, manifest, issues)
  }

  scanPageContract(projectRoot, manifest, issues)
  const errors = issues.filter((entry) => entry.severity === "error")
  const warnings = issues.filter((entry) => entry.severity === "warning")
  return { mode, files, issues, errors, warnings, passed: errors.length === 0 }
}

/** Run the guard and fail closed for unsafe or unowned UI sources. */
export function assertUiGuard(options: UiGuardOptions = {}): UiGuardReport {
  const report = scanUiSource(options)
  if (!report.passed) throw new UiGuardError(report)
  return report
}

/** Vite integration used by production and Storybook builds. */
export function createUiGuardPlugin(options: Omit<UiGuardOptions, "projectRoot"> = {}): Plugin {
  const mode = options.mode ?? uiGuardModeFromEnv()
  return {
    name: "kanban-ui-guard",
    buildStart() {
      const report = scanUiSource({ ...options, mode })
      for (const warning of report.warnings) this.warn(warning.message)
      if (!report.passed) this.error(formatUiGuardIssues(report))
    },
  }
}

function collectSourceFiles(filePath: string, projectRoot: string, forbiddenRoots: readonly string[], files: string[]): void {
  let metadata
  try {
    metadata = lstatSync(filePath)
  } catch {
    return
  }
  if (metadata.isSymbolicLink() || isForbiddenPath(filePath, projectRoot, forbiddenRoots)) return
  if (metadata.isFile()) {
    if (isSourceFile(filePath) && !isTestFile(filePath)) files.push(filePath)
    return
  }
  if (!metadata.isDirectory()) return
  for (const entry of readdirSync(filePath, { withFileTypes: true })) {
    if (entry.name === "node_modules" || entry.name === "dist" || entry.name === "output") continue
    collectSourceFiles(path.join(filePath, entry.name), projectRoot, forbiddenRoots, files)
  }
}

function scanSourceText(source: string, relativePath: string, mode: UiGuardMode, manifest: UiGuardManifest, issues: UiGuardIssue[]): void {
  const sharedStyleOwner = relativePath === manifest.tailwindBridge.staticCssEntry
  for (const candidate of collectStaticImports(source, relativePath)) {
    const imported = candidate.value
    if (isCssSpecifier(imported)) scanCssImportEdge(relativePath, imported, mode, manifest, issues)
    if (manifest.unsafeDirectImports.includes(imported) && !sharedStyleOwner) {
      const severity = unsafeImportSeverity(relativePath, imported, mode, manifest)
      issues.push(issue("unsafe-direct-import", severity, relativePath, lineForOffset(source, candidate.offset), `Unsafe direct import ${imported}; migrate this owner to the shared Astryx/CSP path.`))
    }
    if (isForbiddenImport(imported, manifest.forbiddenRoots)) {
      issues.push(issue("forbidden-import", "error", relativePath, lineForOffset(source, candidate.offset), `Import points at a forbidden reference/output path: ${imported}`))
    }
  }

  if (relativePath.endsWith(".css")) {
    for (const candidate of collectCssReferences(source)) scanCssImportEdge(relativePath, candidate.value, mode, manifest, issues)
  }

  scanUnsafeBarrelImports(source, relativePath, mode, manifest, issues)

  const syntax = inspectSourceSyntax(source, relativePath)
  for (const violation of syntax.styleViolations) {
    issues.push(issue(violation.code, "error", relativePath, lineForOffset(source, violation.offset), violation.message))
  }
  if (syntax.rawLayoutCount > 0) scanLegacyRawLayout(syntax.rawLayoutCount, relativePath, mode, manifest, issues)
}

type StaticImport = { readonly value: string; readonly offset: number }

function collectStaticImports(source: string, relativePath: string): readonly StaticImport[] {
  const extension = path.extname(relativePath).toLowerCase()
  if (extension === ".css") return []
  if (![".js", ".jsx", ".mjs", ".cjs", ".ts", ".tsx"].includes(extension)) return []
  const scriptKind = extension === ".tsx" ? ts.ScriptKind.TSX : extension === ".jsx" ? ts.ScriptKind.JSX : ts.ScriptKind.TS
  const sourceFile = ts.createSourceFile(relativePath, source, ts.ScriptTarget.Latest, true, scriptKind)
  const imports: StaticImport[] = []
  const visit = (node: ts.Node): void => {
    if (ts.isImportDeclaration(node) && ts.isStringLiteral(node.moduleSpecifier)) {
      imports.push({ value: node.moduleSpecifier.text, offset: node.moduleSpecifier.getStart(sourceFile) })
    } else if (ts.isExportDeclaration(node) && node.moduleSpecifier !== undefined && ts.isStringLiteral(node.moduleSpecifier)) {
      imports.push({ value: node.moduleSpecifier.text, offset: node.moduleSpecifier.getStart(sourceFile) })
    } else if (ts.isCallExpression(node) && node.arguments.length > 0 && ts.isStringLiteral(node.arguments[0])) {
      const isDynamicImport = node.expression.kind === ts.SyntaxKind.ImportKeyword
      const isRequire = ts.isIdentifier(node.expression) && node.expression.text === "require"
      if (isDynamicImport || isRequire) imports.push({ value: node.arguments[0].text, offset: node.arguments[0].getStart(sourceFile) })
    }
    ts.forEachChild(node, visit)
  }
  visit(sourceFile)
  return imports
}

function collectCssReferences(source: string): readonly StaticImport[] {
  const references: StaticImport[] = []
  for (const match of source.matchAll(/@import\s+(?:url\(\s*)?["']?([^"'\s)]+)["']?\s*\)?/g)) {
    references.push({ value: match[1], offset: match.index ?? 0 })
  }
  for (const match of source.matchAll(/url\(\s*["']?([^"'\s)]+)["']?\s*\)/g)) {
    references.push({ value: match[1], offset: match.index ?? 0 })
  }
  return references
}

function isCssSpecifier(imported: string): boolean {
  return imported.endsWith(".css") || imported.includes(".css?") || imported.includes(".css#")
}

function scanCssImportEdge(importer: string, imported: string, mode: UiGuardMode, manifest: UiGuardManifest, issues: UiGuardIssue[]): void {
  const pathValue = canonicalCssImportPath(importer, imported)
  if (pathValue === undefined || manifest.cssImports.some((entry) => entry.importer === importer && entry.path === pathValue)) return
  issues.push(issue("css-import", mode === "enforce" ? "error" : "warning", importer, undefined, `CSS import is not in the machine baseline: ${importer} -> ${pathValue}`))
}

function unsafeImportSeverity(importer: string, imported: string, mode: UiGuardMode, manifest: UiGuardManifest): UiGuardIssue["severity"] {
  if (mode !== "enforce") return "warning"
  return manifest.unsafeImportBaseline.some((entry) => entry.importer === importer && entry.path === imported) ? "warning" : "error"
}

function canonicalCssImportPath(importer: string, imported: string): string | undefined {
  const clean = imported.split(/[?#]/, 1)[0]
  if (!clean || clean.startsWith("data:") || clean.startsWith("#")) return undefined
  if (!clean.startsWith(".")) return clean
  return toManifestPath(path.posix.normalize(path.posix.join(path.posix.dirname(importer), clean)))
}

type SourceSyntaxInspection = {
  readonly rawLayoutCount: number
  readonly styleViolations: readonly { readonly code: "inline-style" | "dom-style"; readonly offset: number; readonly message: string }[]
}

const layoutClassNames = new Set([
  "absolute",
  "block",
  "column",
  "container",
  "contents",
  "fixed",
  "flex",
  "flow-root",
  "grid",
  "inline-block",
  "inline-flex",
  "inline-grid",
  "layout",
  "relative",
  "row",
  "stack",
  "sticky",
  "wrap",
  "wrapper",
])
const layoutClassPrefixes = [
  "aspect-",
  "basis-",
  "bottom-",
  "col-",
  "content-",
  "flex-",
  "gap-",
  "grid-",
  "h-",
  "inset-",
  "items-",
  "justify-",
  "left-",
  "m-",
  "max-",
  "mb-",
  "min-",
  "ml-",
  "mr-",
  "mt-",
  "mx-",
  "my-",
  "order-",
  "overflow-",
  "p-",
  "pb-",
  "place-",
  "pl-",
  "pr-",
  "pt-",
  "px-",
  "py-",
  "right-",
  "row-",
  "space-",
  "top-",
  "w-",
  "z-",
]

function inspectSourceSyntax(source: string, relativePath: string): SourceSyntaxInspection {
  const extension = path.extname(relativePath).toLowerCase()
  if (![".js", ".jsx", ".mjs", ".cjs", ".ts", ".tsx"].includes(extension)) {
    return { rawLayoutCount: 0, styleViolations: [] }
  }
  const scriptKind = extension === ".tsx" ? ts.ScriptKind.TSX : extension === ".jsx" ? ts.ScriptKind.JSX : ts.ScriptKind.TS
  const sourceFile = ts.createSourceFile(relativePath, source, ts.ScriptTarget.Latest, true, scriptKind)
  let rawDivCount = 0
  let layoutSpanCount = 0
  const styleViolations: { code: "inline-style" | "dom-style"; offset: number; message: string }[] = []
  const seenStyleOffsets = new Set<number>()

  const addViolation = (code: "inline-style" | "dom-style", node: ts.Node, message: string): void => {
    const offset = node.getStart(sourceFile)
    if (seenStyleOffsets.has(offset)) return
    seenStyleOffsets.add(offset)
    styleViolations.push({ code, offset, message })
  }
  const inspectJsxTag = (tag: ts.JsxOpeningLikeElement): void => {
    const tagName = tag.tagName.getText(sourceFile)
    if (tagName === "div") rawDivCount += 1
    if (tagName === "span" && hasLayoutClass(tag.attributes, sourceFile)) layoutSpanCount += 1
    if (tagName === "style") addViolation("inline-style", tag, "Inline style tags/props are forbidden; use Astryx, Tailwind, or a static stylesheet.")
    for (const attribute of tag.attributes.properties) {
      if (ts.isJsxAttribute(attribute) && jsxAttributeNameText(attribute.name) === "style") {
        addViolation("inline-style", attribute, "Inline style tags/props are forbidden; use Astryx, Tailwind, or a static stylesheet.")
      }
      if (ts.isJsxAttribute(attribute) && jsxAttributeNameText(attribute.name) === "dangerouslySetInnerHTML") {
        addViolation("inline-style", attribute, "dangerouslySetInnerHTML is forbidden under the static CSP style policy.")
      }
      if (ts.isJsxSpreadAttribute(attribute) && spreadMayCarryStyle(attribute.expression, sourceFile)) {
        addViolation("inline-style", attribute, "JSX spread props may inject inline style; use explicit static props instead.")
      }
    }
  }

  const visit = (node: ts.Node): void => {
    if (ts.isJsxElement(node)) inspectJsxTag(node.openingElement)
    else if (ts.isJsxSelfClosingElement(node)) inspectJsxTag(node)
    if (ts.isBinaryExpression(node) && isAssignmentOperator(node.operatorToken.kind)) {
      if (expressionContainsStyle(node.left)) addViolation("dom-style", node.left, "DOM .style mutation is forbidden under the static CSP style policy.")
      else if (expressionContainsMarkupSink(node.left)) addViolation("inline-style", node.left, "Runtime HTML/style injection is forbidden under the static CSP style policy.")
    }
    if (ts.isCallExpression(node) && isStyleMutationCall(node)) {
      addViolation("dom-style", node.expression, "DOM .style mutation is forbidden under the static CSP style policy.")
    }
    if (ts.isCallExpression(node) && isMarkupInjectionCall(node)) {
      addViolation("inline-style", node.expression, "Runtime HTML/style injection is forbidden under the static CSP style policy.")
    }
    if (ts.isCallExpression(node) && isStyleElementFactory(node)) {
      addViolation("inline-style", node.expression, "Runtime style element injection is forbidden under the static CSP style policy.")
    }
    if (ts.isCallExpression(node) && isCreateElementCall(node) && createElementPropsMayCarryStyle(node.arguments[1])) {
      addViolation("inline-style", node.expression, "React.createElement style props are forbidden under the static CSP style policy.")
    }
    if (ts.isCallExpression(node) && isCreateElementCall(node)) {
      const rawLayout = rawLayoutFromCreateElement(node, sourceFile)
      rawDivCount += rawLayout.rawDivCount
      layoutSpanCount += rawLayout.layoutSpanCount
    }
    if (ts.isPropertyAssignment(node) && node.name.getText(sourceFile) === "dangerouslySetInnerHTML") {
      addViolation("inline-style", node.name, "dangerouslySetInnerHTML is forbidden under the static CSP style policy.")
    }
    ts.forEachChild(node, visit)
  }
  visit(sourceFile)
  return { rawLayoutCount: rawDivCount + layoutSpanCount, styleViolations }
}

function scanUnsafeBarrelImports(source: string, relativePath: string, mode: UiGuardMode, manifest: UiGuardManifest, issues: UiGuardIssue[]): void {
  const extension = path.extname(relativePath).toLowerCase()
  if (![".js", ".jsx", ".mjs", ".cjs", ".ts", ".tsx"].includes(extension)) return
  const scriptKind = extension === ".tsx" ? ts.ScriptKind.TSX : extension === ".jsx" ? ts.ScriptKind.JSX : ts.ScriptKind.TS
  const sourceFile = ts.createSourceFile(relativePath, source, ts.ScriptTarget.Latest, true, scriptKind)
  const unsafeComponents = new Set(manifest.unsafeDirectImports.map((entry) => entry.slice(entry.lastIndexOf("/") + 1)))
  const sharedStyleOwner = relativePath === manifest.tailwindBridge.staticCssEntry
  const namespaceAliases = new Set<string>()
  const reported = new Set<string>()

  const reportUnsafe = (component: string, importedPath: string, node: ts.Node): void => {
    if (!unsafeComponents.has(component) || sharedStyleOwner) return
    const key = `${component}:${node.getStart(sourceFile)}`
    if (reported.has(key)) return
    reported.add(key)
    issues.push(issue("unsafe-direct-import", unsafeImportSeverity(relativePath, importedPath, mode, manifest), relativePath, lineForOffset(source, node.getStart(sourceFile)), `Unsafe Astryx component access ${component} via ${importedPath}; migrate this owner to the shared Astryx/CSP path.`))
  }

  const reportUnsafeStar = (node: ts.Node): void => {
    if (sharedStyleOwner) return
    const key = `*:${node.getStart(sourceFile)}`
    if (reported.has(key)) return
    reported.add(key)
    issues.push(issue("unsafe-direct-import", mode === "enforce" ? "error" : "warning", relativePath, lineForOffset(source, node.getStart(sourceFile)), "Wildcard Astryx barrel re-export may expose unsafe components; use explicit safe exports."))
  }

  const memberName = (node: ts.PropertyAccessExpression | ts.ElementAccessExpression): string | undefined => {
    if (ts.isPropertyAccessExpression(node)) return node.name.text
    return node.argumentExpression !== undefined && ts.isStringLiteral(node.argumentExpression) ? node.argumentExpression.text : undefined
  }

  const visit = (node: ts.Node): void => {
    if (ts.isVariableDeclaration(node) && ts.isIdentifier(node.name) && node.initializer !== undefined && isBareCoreLoaderExpression(node.initializer)) {
      namespaceAliases.add(node.name.text)
    }
    if (ts.isImportDeclaration(node) && ts.isStringLiteral(node.moduleSpecifier) && node.moduleSpecifier.text === "@astryxdesign/core") {
      const bindings = node.importClause?.namedBindings
      if (bindings !== undefined && ts.isNamespaceImport(bindings)) {
        namespaceAliases.add(bindings.name.text)
      } else if (bindings !== undefined && ts.isNamedImports(bindings)) {
        for (const element of bindings.elements) {
          reportUnsafe(element.propertyName?.text ?? element.name.text, `@astryxdesign/core/${element.propertyName?.text ?? element.name.text}`, element)
        }
      } else if (bindings === undefined) {
        reportUnsafeStar(node)
      }
    } else if (ts.isExportDeclaration(node) && node.moduleSpecifier !== undefined && ts.isStringLiteral(node.moduleSpecifier) && node.moduleSpecifier.text === "@astryxdesign/core") {
      const clause = node.exportClause
      if (clause === undefined) {
        reportUnsafeStar(node)
      } else if (ts.isNamespaceExport(clause)) {
        reportUnsafeStar(node)
        namespaceAliases.add(clause.name.text)
      } else if (ts.isNamedExports(clause)) {
        for (const element of clause.elements) {
          reportUnsafe(element.propertyName?.text ?? element.name.text, `@astryxdesign/core/${element.propertyName?.text ?? element.name.text}`, element)
        }
      }
    }
    if (ts.isCallExpression(node) && isBareCoreLoader(node)) reportUnsafeStar(node)
    if (ts.isPropertyAccessExpression(node) || ts.isElementAccessExpression(node)) {
      const component = memberName(node)
      if (component !== undefined && ts.isIdentifier(node.expression) && namespaceAliases.has(node.expression.text)) {
        reportUnsafe(component, "@astryxdesign/core", node)
      }
      if (component !== undefined && isBareCoreLoaderExpression(node.expression)) {
        reportUnsafe(component, "@astryxdesign/core", node)
      }
    }
    ts.forEachChild(node, visit)
  }
  if (!sharedStyleOwner) visit(sourceFile)
}

function isBareCoreLoader(node: ts.CallExpression): boolean {
  if (node.arguments.length === 0 || !ts.isStringLiteral(node.arguments[0]) || node.arguments[0].text !== "@astryxdesign/core") return false
  return node.expression.kind === ts.SyntaxKind.ImportKeyword || (ts.isIdentifier(node.expression) && node.expression.text === "require")
}

function isBareCoreLoaderExpression(expression: ts.Expression): boolean {
  let candidate = expression
  while (ts.isParenthesizedExpression(candidate) || ts.isAwaitExpression(candidate)) candidate = candidate.expression
  return ts.isCallExpression(candidate) && isBareCoreLoader(candidate)
}

function isAssignmentOperator(kind: ts.SyntaxKind): boolean {
  return kind >= ts.SyntaxKind.FirstAssignment && kind <= ts.SyntaxKind.LastAssignment
}

function expressionContainsStyle(expression: ts.Expression): boolean {
  if (ts.isParenthesizedExpression(expression)) return expressionContainsStyle(expression.expression)
  if (ts.isPropertyAccessExpression(expression)) return expression.name.text === "style" || expressionContainsStyle(expression.expression)
  if (ts.isElementAccessExpression(expression)) {
    return (expression.argumentExpression !== undefined && ts.isStringLiteral(expression.argumentExpression) && expression.argumentExpression.text === "style") || expressionContainsStyle(expression.expression)
  }
  return false
}

function expressionContainsMarkupSink(expression: ts.Expression): boolean {
  if (ts.isParenthesizedExpression(expression)) return expressionContainsMarkupSink(expression.expression)
  if (ts.isPropertyAccessExpression(expression)) {
    return expression.name.text === "innerHTML" || expression.name.text === "outerHTML" || expressionContainsMarkupSink(expression.expression)
  }
  if (ts.isElementAccessExpression(expression)) {
    return (expression.argumentExpression !== undefined && ts.isStringLiteral(expression.argumentExpression) && (expression.argumentExpression.text === "innerHTML" || expression.argumentExpression.text === "outerHTML")) || expressionContainsMarkupSink(expression.expression)
  }
  return false
}

function isStyleMutationCall(node: ts.CallExpression): boolean {
  if (ts.isPropertyAccessExpression(node.expression)) {
    if ((node.expression.name.text === "setProperty" || node.expression.name.text === "removeProperty") && expressionContainsStyle(node.expression.expression)) return true
    if ((node.expression.name.text === "setAttribute" || node.expression.name.text === "removeAttribute") && firstArgumentIs(node, "style")) return true
    return node.expression.name.text === "assign" && ts.isIdentifier(node.expression.expression) && node.expression.expression.text === "Object" && node.arguments.length > 0 && expressionContainsStyle(node.arguments[0])
  }
  return false
}

function isMarkupInjectionCall(node: ts.CallExpression): boolean {
  return ts.isPropertyAccessExpression(node.expression) && node.expression.name.text === "insertAdjacentHTML"
}

function firstArgumentIs(node: ts.CallExpression, value: string): boolean {
  return node.arguments.length > 0 && ts.isStringLiteral(node.arguments[0]) && node.arguments[0].text === value
}

function hasLayoutClass(attributes: ts.JsxAttributes, sourceFile: ts.SourceFile): boolean {
  const classAttribute = attributes.properties.find((attribute): attribute is ts.JsxAttribute => ts.isJsxAttribute(attribute) && jsxAttributeNameText(attribute.name) === "className")
  if (classAttribute?.initializer === undefined) return false
  const value = ts.isStringLiteral(classAttribute.initializer)
    ? classAttribute.initializer.text
    : ts.isJsxExpression(classAttribute.initializer) && classAttribute.initializer.expression !== undefined
      ? classAttribute.initializer.expression.getText(sourceFile)
      : ""
  const tokens = value.match(/[A-Za-z0-9_-]+/g) ?? []
  if (tokens.some((token) => layoutClassNames.has(token) || layoutClassPrefixes.some((prefix) => token.startsWith(prefix)))) return true
  return attributes.properties.some((attribute) => ts.isJsxSpreadAttribute(attribute) && (spreadObjectHasLayoutClass(attribute.expression, sourceFile) || !ts.isObjectLiteralExpression(attribute.expression)))
}

function spreadMayCarryStyle(expression: ts.Expression, sourceFile: ts.SourceFile): boolean {
  if (ts.isObjectLiteralExpression(expression)) {
    return expression.properties.some((property) => {
      if (!ts.isPropertyAssignment(property)) return false
      const name = property.name.getText(sourceFile)
      return name === "style" || name === "dangerouslySetInnerHTML"
    })
  }
  return ts.isIdentifier(expression) && /style/i.test(expression.text)
}

function spreadObjectHasLayoutClass(expression: ts.Expression, sourceFile: ts.SourceFile): boolean {
  if (!ts.isObjectLiteralExpression(expression)) return false
  const classProperty = expression.properties.find((property): property is ts.PropertyAssignment => ts.isPropertyAssignment(property) && property.name.getText(sourceFile) === "className")
  if (classProperty === undefined) return false
  const tokens = classProperty.initializer.getText(sourceFile).match(/[A-Za-z0-9_-]+/g) ?? []
  return tokens.some((token) => layoutClassNames.has(token) || layoutClassPrefixes.some((prefix) => token.startsWith(prefix)))
}

function jsxAttributeNameText(name: ts.JsxAttributeName): string | undefined {
  return ts.isIdentifier(name) ? name.text : undefined
}

function isStyleElementFactory(node: ts.CallExpression): boolean {
  if (node.arguments.length === 0 || !ts.isStringLiteral(node.arguments[0]) || node.arguments[0].text !== "style") return false
  if (ts.isIdentifier(node.expression)) return node.expression.text === "createElement"
  return ts.isPropertyAccessExpression(node.expression) && node.expression.name.text === "createElement"
}

function isCreateElementCall(node: ts.CallExpression): boolean {
  return ts.isIdentifier(node.expression) && node.expression.text === "createElement"
    || ts.isPropertyAccessExpression(node.expression) && node.expression.name.text === "createElement"
}

function createElementPropsMayCarryStyle(argument: ts.Expression | undefined): boolean {
  if (argument === undefined) return false
  if (ts.isObjectLiteralExpression(argument)) {
    return argument.properties.some((property) => ts.isPropertyAssignment(property) && ["style", "dangerouslySetInnerHTML"].includes(property.name.getText()))
  }
  return ts.isIdentifier(argument) && /style|props/i.test(argument.text)
}

function rawLayoutFromCreateElement(node: ts.CallExpression, sourceFile: ts.SourceFile): { rawDivCount: number; layoutSpanCount: number } {
  const tag = node.arguments[0]
  const props = node.arguments[1]
  if (tag === undefined) return { rawDivCount: 0, layoutSpanCount: 0 }
  const tagName = ts.isStringLiteral(tag) ? tag.text : undefined
  const hasLayout = props !== undefined && (ts.isObjectLiteralExpression(props) ? props.properties.some((property) => ts.isPropertyAssignment(property) && property.name.getText(sourceFile) === "className" && classExpressionHasLayout(property.initializer, sourceFile)) : ts.isIdentifier(props) && /class|props/i.test(props.text))
  if (tagName === "div" || (tagName === undefined && hasLayout)) return { rawDivCount: 1, layoutSpanCount: 0 }
  if (tagName === "span" && hasLayout) return { rawDivCount: 0, layoutSpanCount: 1 }
  return { rawDivCount: 0, layoutSpanCount: 0 }
}

function classExpressionHasLayout(expression: ts.Expression, sourceFile: ts.SourceFile): boolean {
  const tokens = expression.getText(sourceFile).match(/[A-Za-z0-9_-]+/g) ?? []
  return tokens.some((token) => layoutClassNames.has(token) || layoutClassPrefixes.some((prefix) => token.startsWith(prefix)))
}

function scanLegacyRawLayout(count: number, relativePath: string, mode: UiGuardMode, manifest: UiGuardManifest, issues: UiGuardIssue[]): void {
  const baseline = manifest.legacyRawLayout.find((entry) => entry.path === relativePath)
  if (baseline === undefined) {
    issues.push(issue("legacy-raw-layout-over-budget", "error", relativePath, undefined, `Raw layout source is not present in the Astryx migration baseline (${count} nodes); add an explicit owner and maxCount before continuing.`))
    return
  }
  if (count > baseline.maxCount) {
    issues.push(issue("legacy-raw-layout-over-budget", mode === "enforce" ? "error" : "warning", relativePath, undefined, `Raw layout count exceeds its Astryx migration baseline (${count} > ${baseline.maxCount}); replace raw div/layout spans with Astryx primitives or utilities.`))
  } else if (count > 0) {
    issues.push(issue("legacy-raw-layout", "warning", relativePath, undefined, `Raw layout remains allowlisted for ${baseline.owner}; exit: ${baseline.exitCondition}`))
  }
}

function scanCssInventory(source: string, relativePath: string, mode: UiGuardMode, manifest: UiGuardManifest, issues: UiGuardIssue[]): void {
  if (manifest.forbiddenRoots.some((root) => sameOrDescendant(relativePath, root))) return
  const owned = manifest.ownedPaths.includes(relativePath)
  const residual = manifest.residualCss.find((entry) => entry.path === relativePath)
  if (!owned && residual === undefined) {
    issues.push(issue("unowned-css", "error", relativePath, undefined, "CSS file is not an owned path or an explicit residual allowlist entry."))
    return
  }
  if (residual === undefined) return
  const loc = lineCount(source)
  if (loc > residual.maxLoc) {
    issues.push(issue("residual-css-over-budget", mode === "enforce" ? "error" : "warning", relativePath, undefined, `Residual CSS exceeds its allowlist budget (${loc} LOC > ${residual.maxLoc} LOC).`))
  } else {
    issues.push(issue("residual-css", "warning", relativePath, undefined, `Legacy CSS remains allowlisted for ${residual.owner}; exit: ${residual.exitCondition}`))
  }
}

function scanPageContract(projectRoot: string, manifest: UiGuardManifest, issues: UiGuardIssue[]): void {
  const sharedPath = path.resolve(projectRoot, manifest.pageContract.sharedStyleEntry)
  if (!isRegularFile(sharedPath)) {
    issues.push(issue("page-contract", "error", manifest.pageContract.sharedStyleEntry, undefined, "The shared static CSS entry is missing."))
  }
  for (const entry of manifest.pageContract.entrypoints) {
    const absolutePath = path.resolve(projectRoot, entry.path)
    if (!isRegularFile(absolutePath)) {
      issues.push(issue("page-contract", "error", entry.path, undefined, "Page contract entrypoint is missing."))
      continue
    }
    const source = readFileSync(absolutePath, "utf8")
    const extension = path.extname(entry.path).toLowerCase()
    const scriptKind = extension === ".tsx" ? ts.ScriptKind.TSX : extension === ".jsx" ? ts.ScriptKind.JSX : ts.ScriptKind.TS
    const sourceFile = ts.createSourceFile(entry.path, source, ts.ScriptTarget.Latest, true, scriptKind)
    let count = 0
    const visit = (node: ts.Node): void => {
      if (ts.isImportDeclaration(node) && ts.isStringLiteral(node.moduleSpecifier) && node.moduleSpecifier.text === entry.import) count += 1
      ts.forEachChild(node, visit)
    }
    visit(sourceFile)
    if (count !== 1) {
      issues.push(issue("page-contract", "error", entry.path, undefined, `Page entrypoint must import the shared CSS entry exactly once (found ${count}).`))
    }
  }
}

function issue(code: UiGuardIssueCode, severity: UiGuardIssue["severity"], issuePath: string | undefined, line: number | undefined, message: string): UiGuardIssue {
  return { code, severity, path: issuePath, line, message }
}

function formatUiGuardIssues(report: UiGuardReport): string {
  if (report.issues.length === 0) return "Astryx UI guard passed"
  return report.issues.map((entry) => {
    const location = entry.path === undefined ? "" : `${entry.path}${entry.line === undefined ? "" : `:${entry.line}`}: `
    return `[${entry.severity}] ${location}${entry.message}`
  }).join("\n")
}

function isSourceFile(filePath: string): boolean {
  return /\.(?:css|cjs|js|jsx|mjs|ts|tsx)$/.test(filePath)
}

function isTestFile(filePath: string): boolean {
  return /(?:\.test|\.spec)\.[^.]+$/.test(filePath)
}

function isRegularFile(filePath: string): boolean {
  try {
    return lstatSync(filePath).isFile()
  } catch {
    return false
  }
}

function isForbiddenPath(filePath: string, projectRoot: string, forbiddenRoots: readonly string[]): boolean {
  const relativePath = toManifestPath(path.relative(projectRoot, filePath))
  return forbiddenRoots.some((root) => sameOrDescendant(relativePath, root))
}

function isForbiddenImport(imported: string, forbiddenRoots: readonly string[]): boolean {
  const candidate = toManifestPath(imported.replace(/^\.\/?/, ""))
  const segments = candidate.split("/").filter(Boolean)
  return forbiddenRoots.some((root) => {
    const rootSegments = root.split("/").filter(Boolean)
    return rootSegments.length > 0 && segments.some((_, index) => rootSegments.every((segment, offset) => segments[index + offset] === segment))
  })
}

function sameOrDescendant(candidate: string, root: string): boolean {
  return candidate === root || candidate.startsWith(`${root}/`)
}

function toManifestPath(value: string): string {
  return value.replaceAll(path.sep, "/").replace(/^\.\//, "")
}

function validateManifestPath(value: string): void {
  if (!isNonEmptyString(value) || path.isAbsolute(value) || value.split(/[\\/]/).includes("..")) {
    throw new Error(`Invalid Astryx UI safety manifest path: ${String(value)}`)
  }
}

function validateCssImportPath(value: string): void {
  if (!isNonEmptyString(value) || value.startsWith("/") || value.includes("\\") || value.split("/").includes("..")) {
    throw new Error(`Invalid Astryx UI safety manifest CSS path: ${String(value)}`)
  }
}

function lineCount(source: string): number {
  return source.length === 0 ? 0 : (source.match(/\n/g)?.length ?? 0) + (source.endsWith("\n") ? 0 : 1)
}

function lineForOffset(source: string, offset: number): number {
  return source.slice(0, offset).split("\n").length
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === "string" && value.trim().length > 0
}

function isStringArray(value: unknown): value is readonly string[] {
  return Array.isArray(value) && value.every((entry) => isNonEmptyString(entry))
}

function isSwizzle(value: unknown): value is UiGuardSwizzle {
  return isRecord(value) && ["component", "ownedPath", "sourcePackage", "sourceVersion", "sourcePath", "command", "sourceSha256", "reason", "publicApi", "license", "runtimeStylePolicy"].every((key) => isNonEmptyString(value[key]))
}

function isResidualCss(value: unknown): value is UiGuardResidualCss {
  return isRecord(value) && isNonEmptyString(value.path) && isSafeNonNegativeInteger(value.maxLoc) && isNonEmptyString(value.owner) && isNonEmptyString(value.reason) && isNonEmptyString(value.exitCondition)
}

function isLegacyRawLayout(value: unknown): value is UiGuardLegacyRawLayout {
  return isRecord(value) && isNonEmptyString(value.path) && isSafeNonNegativeInteger(value.maxCount) && isNonEmptyString(value.owner) && isNonEmptyString(value.exitCondition)
}

function isCssImport(value: unknown): value is UiGuardCssImport {
  return isRecord(value) && isNonEmptyString(value.importer) && isNonEmptyString(value.path)
}

function isUnsafeImportBaseline(value: unknown): value is UiGuardUnsafeImportBaseline {
  return isRecord(value) && isNonEmptyString(value.importer) && isNonEmptyString(value.path)
}

function isPageEntrypoint(value: unknown): value is UiGuardPageEntrypoint {
  return isRecord(value) && isNonEmptyString(value.path) && isNonEmptyString(value.import)
}

function isSafeNonNegativeInteger(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0
}
