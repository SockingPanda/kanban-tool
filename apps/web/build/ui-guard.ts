import { lstatSync, readFileSync, readdirSync } from "node:fs"
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
  readonly ownedPaths: readonly string[]
  readonly forbiddenRoots: readonly string[]
  readonly residualCss: readonly UiGuardResidualCss[]
  readonly legacyRawLayout: readonly UiGuardLegacyRawLayout[]
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
  if (!Array.isArray(value.swizzles) || !value.swizzles.every(isSwizzle)) {
    throw new Error("Astryx UI safety manifest swizzles are invalid")
  }
  if (!Array.isArray(value.residualCss) || !value.residualCss.every(isResidualCss)) {
    throw new Error("Astryx UI safety manifest residualCss is invalid")
  }
  if (!Array.isArray(value.legacyRawLayout) || !value.legacyRawLayout.every(isLegacyRawLayout)) {
    throw new Error("Astryx UI safety manifest legacyRawLayout is invalid")
  }
  if (!isRecord(value.pageContract) || value.pageContract.schemaVersion !== UI_GUARD_SCHEMA_VERSION || !isNonEmptyString(value.pageContract.sharedStyleEntry) || !Array.isArray(value.pageContract.entrypoints) || !value.pageContract.entrypoints.every(isPageEntrypoint)) {
    throw new Error("Astryx UI safety manifest pageContract is invalid")
  }
  for (const candidate of [...value.ownedPaths, ...value.forbiddenRoots, ...value.residualCss.map((item) => item.path), ...value.legacyRawLayout.map((item) => item.path), value.pageContract.sharedStyleEntry, ...value.pageContract.entrypoints.map((item) => item.path)]) {
    validateManifestPath(candidate)
  }
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
    if (manifest.unsafeDirectImports.includes(imported) && !sharedStyleOwner) {
      const severity = manifest.ownedPaths.includes(relativePath) || mode === "enforce" ? "error" : "warning"
      issues.push(issue("unsafe-direct-import", severity, relativePath, lineForOffset(source, candidate.offset), `Unsafe direct import ${imported}; migrate this owner to the shared Astryx/CSP path.`))
    }
    if (isForbiddenImport(imported, manifest.forbiddenRoots)) {
      issues.push(issue("forbidden-import", "error", relativePath, lineForOffset(source, candidate.offset), `Import points at a forbidden reference/output path: ${imported}`))
    }
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
  if (extension === ".css") {
    const imports: StaticImport[] = []
    for (const match of source.matchAll(/@import\s+["']([^"']+)["']/g)) {
      imports.push({ value: match[1], offset: match.index ?? 0 })
    }
    return imports
  }
  if (![".js", ".jsx", ".mjs", ".cjs", ".ts", ".tsx"].includes(extension)) return []
  const scriptKind = extension === ".tsx" ? ts.ScriptKind.TSX : extension === ".jsx" ? ts.ScriptKind.JSX : ts.ScriptKind.TS
  const sourceFile = ts.createSourceFile(relativePath, source, ts.ScriptTarget.Latest, true, scriptKind)
  const imports: StaticImport[] = []
  const visit = (node: ts.Node): void => {
    if (ts.isImportDeclaration(node) && ts.isStringLiteral(node.moduleSpecifier)) {
      imports.push({ value: node.moduleSpecifier.text, offset: node.moduleSpecifier.getStart(sourceFile) })
    } else if (ts.isExportDeclaration(node) && node.moduleSpecifier !== undefined && ts.isStringLiteral(node.moduleSpecifier)) {
      imports.push({ value: node.moduleSpecifier.text, offset: node.moduleSpecifier.getStart(sourceFile) })
    }
    ts.forEachChild(node, visit)
  }
  visit(sourceFile)
  return imports
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
    }
  }

  const visit = (node: ts.Node): void => {
    if (ts.isJsxElement(node)) inspectJsxTag(node.openingElement)
    else if (ts.isJsxSelfClosingElement(node)) inspectJsxTag(node)
    if (ts.isBinaryExpression(node) && isAssignmentOperator(node.operatorToken.kind) && expressionContainsStyle(node.left)) {
      addViolation("dom-style", node.left, "DOM .style mutation is forbidden under the static CSP style policy.")
    }
    if (ts.isCallExpression(node) && isStyleMutationCall(node)) {
      addViolation("dom-style", node.expression, "DOM .style mutation is forbidden under the static CSP style policy.")
    }
    if (ts.isCallExpression(node) && isStyleElementFactory(node)) {
      addViolation("inline-style", node.expression, "Runtime style element injection is forbidden under the static CSP style policy.")
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
  const visit = (node: ts.Node): void => {
    if (ts.isImportDeclaration(node) && ts.isStringLiteral(node.moduleSpecifier) && node.moduleSpecifier.text === "@astryxdesign/core" && node.importClause?.namedBindings !== undefined && ts.isNamedImports(node.importClause.namedBindings)) {
      for (const element of node.importClause.namedBindings.elements) {
        const component = element.propertyName?.text ?? element.name.text
        if (!unsafeComponents.has(component)) continue
        const severity = manifest.ownedPaths.includes(relativePath) || mode === "enforce" ? "error" : "warning"
        issues.push(issue("unsafe-direct-import", severity, relativePath, lineForOffset(source, element.getStart(sourceFile)), `Unsafe barrel import ${component} from @astryxdesign/core; migrate this owner to the shared Astryx/CSP path.`))
      }
    }
    ts.forEachChild(node, visit)
  }
  if (!sharedStyleOwner) visit(sourceFile)
}

function isAssignmentOperator(kind: ts.SyntaxKind): boolean {
  return kind >= ts.SyntaxKind.FirstAssignment && kind <= ts.SyntaxKind.LastAssignment
}

function expressionContainsStyle(expression: ts.Expression): boolean {
  if (ts.isParenthesizedExpression(expression)) return expressionContainsStyle(expression.expression)
  if (ts.isPropertyAccessExpression(expression)) return expression.name.text === "style" || expressionContainsStyle(expression.expression)
  if (ts.isElementAccessExpression(expression)) {
    return (ts.isStringLiteral(expression.argumentExpression) && expression.argumentExpression.text === "style") || expressionContainsStyle(expression.expression)
  }
  return false
}

function isStyleMutationCall(node: ts.CallExpression): boolean {
  if (ts.isPropertyAccessExpression(node.expression)) {
    if ((node.expression.name.text === "setProperty" || node.expression.name.text === "removeProperty") && expressionContainsStyle(node.expression.expression)) return true
    return node.expression.name.text === "assign" && ts.isIdentifier(node.expression.expression) && node.expression.expression.text === "Object" && node.arguments.length > 0 && expressionContainsStyle(node.arguments[0])
  }
  return false
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
    issues.push(issue("residual-css-over-budget", "error", relativePath, undefined, `Residual CSS exceeds its allowlist budget (${loc} LOC > ${residual.maxLoc} LOC).`))
  } else {
    issues.push(issue("residual-css", mode === "enforce" ? "error" : "warning", relativePath, undefined, `Legacy CSS remains allowlisted for ${residual.owner}; exit: ${residual.exitCondition}`))
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
    const count = source.split(entry.import).length - 1
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

function isPageEntrypoint(value: unknown): value is UiGuardPageEntrypoint {
  return isRecord(value) && isNonEmptyString(value.path) && isNonEmptyString(value.import)
}

function isSafeNonNegativeInteger(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0
}
