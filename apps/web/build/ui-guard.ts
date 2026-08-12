import { createHash } from "node:crypto"
import { lstatSync, readFileSync, readdirSync, realpathSync } from "node:fs"
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

export type UiGuardManifestOrigin =
  | "app-owned"
  | "astryx-reimplementation"
  | "astryx-facade"
  | "astryx-swizzle"

export type UiGuardRootBarrel = {
  readonly path: string
  readonly mode: "explicit"
}

export type UiGuardUpstream = {
  readonly import: string
  readonly package: string
  readonly version: string
  readonly license: string
  readonly sourcePath: string
  readonly sourceSha256: string
}

export type UiGuardPublicSource = {
  readonly exported: string
  readonly imported: string
  readonly sourcePath: string
  readonly kind: "runtime" | "type"
}

export type UiGuardCliEvidence = {
  readonly package: string
  readonly version: string
  readonly command: string
  readonly component: string
  readonly evidenceStdout: string
  readonly evidenceSha256: string
}

export type UiGuardSafeWrapperMember = {
  readonly component: string
  readonly publicApi: readonly string[]
  readonly publicSources: readonly UiGuardPublicSource[]
  readonly upstream?: UiGuardUpstream
  readonly cli?: UiGuardCliEvidence
}

export type UiGuardSafeWrapperComponent = {
  readonly kind: "component"
  readonly component: string
  readonly publicApi: readonly string[]
  readonly publicSources: readonly UiGuardPublicSource[]
  readonly ownedPath: string
  readonly origin: UiGuardManifestOrigin
  readonly reason: string
  readonly runtimeStylePolicy: string
  readonly upstream?: UiGuardUpstream
  readonly cli?: UiGuardCliEvidence
}

export type UiGuardSafeWrapperGroup = {
  readonly kind: "group"
  readonly component: string
  readonly ownedPath: string
  readonly origin: UiGuardManifestOrigin
  readonly reason: string
  readonly runtimeStylePolicy: string
  readonly members: readonly UiGuardSafeWrapperMember[]
}

export type UiGuardSafeWrapper = UiGuardSafeWrapperComponent | UiGuardSafeWrapperGroup

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
  readonly rootBarrel: UiGuardRootBarrel
  readonly safeWrappers: readonly UiGuardSafeWrapper[]
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
  assertExactKeys(
    value,
    [
      "schemaVersion",
      "source",
      "tailwindBridge",
      "rootBarrel",
      "safeWrappers",
      "swizzles",
      "unsafeDirectImports",
      "unsafeImportBaseline",
      "ownedPaths",
      "forbiddenRoots",
      "residualCss",
      "legacyRawLayout",
      "cssImports",
      "pageContract",
    ],
    "Astryx UI safety manifest",
  )
  if (value.schemaVersion !== UI_GUARD_SCHEMA_VERSION) {
    throw new Error(`Unsupported Astryx UI safety manifest schemaVersion: ${String(value.schemaVersion)}`)
  }
  const source = value.source
  if (!isRecord(source)) throw new Error("Astryx UI safety manifest source must be an object")
  assertExactKeys(source, ["package", "version", "tarballOrSourceHash", "license", "upstream"], "Astryx UI safety manifest source")
  for (const key of ["package", "version", "tarballOrSourceHash", "license", "upstream"] as const) {
    if (!isNonEmptyString(source[key])) throw new Error(`Astryx UI safety manifest source.${key} must be a string`)
  }
  if (!isPackageName(source.package)) throw new Error(`Astryx UI safety manifest source.package is invalid: ${source.package}`)

  const bridge = value.tailwindBridge
  if (!isRecord(bridge) || !isStringArray(bridge.packages) || bridge.packages.length === 0 || !isNonEmptyString(bridge.version) || !isNonEmptyString(bridge.staticCssEntry)) {
    throw new Error("Astryx UI safety manifest tailwindBridge is invalid")
  }
  if (isRecord(bridge)) assertExactKeys(bridge, ["packages", "version", "staticCssEntry"], "Astryx UI safety manifest tailwindBridge")
  const rootBarrel = value.rootBarrel
  if (!isRecord(rootBarrel) || rootBarrel.mode !== "explicit" || !isNonEmptyString(rootBarrel.path)) {
    throw new Error("Astryx UI safety manifest rootBarrel is invalid")
  }
  if (isRecord(rootBarrel)) assertExactKeys(rootBarrel, ["path", "mode"], "Astryx UI safety manifest rootBarrel")
  if (!Array.isArray(value.safeWrappers) || value.safeWrappers.length === 0 || !value.safeWrappers.every(isSafeWrapper)) {
    throw new Error("Astryx UI safety manifest safeWrappers are invalid")
  }
  assertSafeWrapperUniqueness(value.safeWrappers)
  if (!isStringArray(value.unsafeDirectImports) || !isStringArray(value.ownedPaths) || !isStringArray(value.forbiddenRoots)) {
    throw new Error("Astryx UI safety manifest path lists are invalid")
  }
  assertUniqueStrings(value.unsafeDirectImports, "unsafeDirectImports")
  assertUniqueStrings(value.ownedPaths, "ownedPaths")
  assertUniqueStrings(value.forbiddenRoots, "forbiddenRoots")
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
  if (isRecord(value.pageContract)) {
    assertExactKeys(value.pageContract, ["schemaVersion", "sharedStyleEntry", "entrypoints"], "Astryx UI safety manifest pageContract")
  }
  for (const candidate of [...value.ownedPaths, ...value.forbiddenRoots, ...value.residualCss.map((item) => item.path), ...value.legacyRawLayout.map((item) => item.path), value.pageContract.sharedStyleEntry, ...value.pageContract.entrypoints.map((item) => item.path), ...value.cssImports.map((item) => item.importer), ...value.unsafeImportBaseline.map((item) => item.importer)]) {
    validateManifestPath(candidate)
  }
  validateManifestPath(rootBarrel.path)
  for (const wrapper of value.safeWrappers) {
    validateManifestPath(wrapper.ownedPath)
    const members = wrapper.kind === "group" ? wrapper.members : [wrapper]
    for (const member of members) {
      for (const publicSource of member.publicSources) validateManifestPath(publicSource.sourcePath)
      if (member.upstream !== undefined) validateManifestPath(member.upstream.sourcePath)
    }
  }
  for (const swizzle of value.swizzles) validateManifestPath(swizzle.ownedPath)
  for (const entry of value.cssImports) validateCssImportPath(entry.path)
  for (const entry of value.unsafeImportBaseline) {
    validateModuleSpecifier(entry.path, "unsafe import path")
  }
  for (const entry of value.unsafeDirectImports) validateModuleSpecifier(entry, "unsafe direct import")
}

/** Validate provenance against the checked-in package metadata and lockfile before scanning sources. */
export function validateUiGuardManifestProvenance(manifest: UiGuardManifest, projectRoot: string): void {
  const packageJsonPath = path.join(projectRoot, "package.json")
  const lockfilePath = path.resolve(projectRoot, "..", "..", "pnpm-lock.yaml")
  let packageJson: {
    dependencies?: Record<string, unknown>
    devDependencies?: Record<string, unknown>
    exports?: Record<string, unknown>
  }
  try {
    packageJson = JSON.parse(readFileSync(packageJsonPath, "utf8")) as typeof packageJson
  } catch (error) {
    throw new Error(`Unable to validate Astryx manifest provenance: ${packageJsonPath}`, { cause: error })
  }

  const installedPackageRoot = resolveInstalledPackageRoot(projectRoot, manifest.source.package)
  const corePackagePath = path.join(installedPackageRoot, "package.json")
  let corePackage: { name?: unknown; version?: unknown; license?: unknown; exports?: Record<string, unknown> }
  try {
    corePackage = JSON.parse(readFileSync(corePackagePath, "utf8")) as typeof corePackage
  } catch (error) {
    throw new Error(`Unable to validate Astryx package provenance: ${corePackagePath}`, { cause: error })
  }
  if (corePackage.name !== manifest.source.package || corePackage.version !== manifest.source.version || corePackage.license !== manifest.source.license) {
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
  if (!/^sha512-[A-Za-z0-9+/]+={0,2}$/.test(manifest.source.tarballOrSourceHash)) {
    throw new Error("Astryx manifest source.tarballOrSourceHash must be a valid SRI hash")
  }
  const owned = new Set<string>()
  const canonicalRoot = realpathSync(projectRoot)
  for (const ownedPath of manifest.ownedPaths) {
    if (owned.has(ownedPath)) throw new Error(`Astryx manifest ownedPaths contains a duplicate: ${ownedPath}`)
    owned.add(ownedPath)
    assertAppFile(projectRoot, canonicalRoot, ownedPath, "ownedPath")
  }

  assertAppFile(projectRoot, canonicalRoot, manifest.rootBarrel.path, "rootBarrel path")
  validateRootBarrelContract(projectRoot, manifest.rootBarrel)

  const wrapperComponents = new Set<string>()
  const wrapperPublicApi = new Set<string>()
  const wrapperRuntimeApi = new Set<string>()
  const wrapperOwnedPaths = new Set<string>()
  const swizzleWrappers = new Map<string, { readonly ownedPath: string; readonly publicApi: readonly string[]; readonly upstream?: UiGuardUpstream; readonly cli?: UiGuardCliEvidence }>()
  const wrapperPublicSources: UiGuardPublicSource[] = []
  for (const wrapper of manifest.safeWrappers) {
    if (wrapperComponents.has(wrapper.component)) throw new Error(`Astryx manifest safeWrappers contains a duplicate component: ${wrapper.component}`)
    if (wrapper.kind === "group") wrapperComponents.add(wrapper.component)
    if (wrapperOwnedPaths.has(wrapper.ownedPath)) throw new Error(`Astryx manifest safeWrappers contains a duplicate ownedPath: ${wrapper.ownedPath}`)
    wrapperOwnedPaths.add(wrapper.ownedPath)
    assertAppFile(projectRoot, canonicalRoot, wrapper.ownedPath, "safeWrapper ownedPath")
    const members = wrapper.kind === "group" ? wrapper.members : [wrapper]
    for (const member of members) {
      if (wrapperComponents.has(member.component)) throw new Error(`Astryx manifest safeWrappers contains a duplicate component: ${member.component}`)
      wrapperComponents.add(member.component)
      for (const publicName of member.publicApi) {
        if (wrapperPublicApi.has(publicName)) throw new Error(`Astryx manifest safeWrappers contains a duplicate publicApi symbol: ${publicName}`)
        wrapperPublicApi.add(publicName)
      }
      if (!sameStringSet(member.publicApi, member.publicSources.map((entry) => entry.exported))) {
        throw new Error(`Astryx safeWrapper publicSources must exactly match publicApi: ${member.component}`)
      }
      const sourceNames = new Set<string>()
      for (const publicSource of member.publicSources) {
        if (sourceNames.has(publicSource.exported)) throw new Error(`Astryx safeWrapper contains a duplicate publicSources symbol: ${publicSource.exported}`)
        sourceNames.add(publicSource.exported)
        if (!isOwnedPublicSource(wrapper, member, publicSource)) {
          throw new Error(`Astryx safeWrapper publicSources source must stay with its owner: ${publicSource.exported}`)
        }
        assertAppFile(projectRoot, canonicalRoot, publicSource.sourcePath, "safeWrapper public source")
        wrapperPublicSources.push(publicSource)
      }
      wrapperRuntimeApi.add(member.component)
      if (member.upstream !== undefined) validateUpstreamProvenance(manifest, corePackage, installedPackageRoot, member.upstream)
      if (member.cli !== undefined) validateCliProvenance(projectRoot, member.cli)
      if (wrapper.origin === "astryx-swizzle") swizzleWrappers.set(member.component, { ownedPath: wrapper.ownedPath, publicApi: member.publicApi, upstream: member.upstream, cli: member.cli })
    }
  }
  const barrelExports = readRootBarrelPublicApi(projectRoot, manifest.rootBarrel.path)
  if (!sameStringSet(barrelExports.names, wrapperPublicApi)) {
    throw new Error("Astryx root barrel exports must exactly match safeWrappers publicApi")
  }
  if (!sameStringSet(barrelExports.runtimeNames, wrapperRuntimeApi)) throw new Error("Astryx root barrel runtime exports must exactly match safeWrapper components")
  if (!samePublicSourceSet(barrelExports.entries, wrapperPublicSources)) {
    throw new Error("Astryx root barrel export modules must exactly match safeWrapper publicSources")
  }

  const legacySwizzles = new Map<string, UiGuardSwizzle>()
  for (const swizzle of manifest.swizzles) {
    if (legacySwizzles.has(swizzle.component)) throw new Error(`Astryx manifest swizzles contains a duplicate component: ${swizzle.component}`)
    legacySwizzles.set(swizzle.component, swizzle)
    if (!owned.has(swizzle.ownedPath)) {
      throw new Error(`Astryx manifest swizzle ownedPath must be listed in ownedPaths: ${swizzle.component}`)
    }
    if (!/^[a-f0-9]{64}$/i.test(swizzle.sourceSha256)) {
      throw new Error(`Astryx manifest swizzle sourceSha256 must be 64 hexadecimal characters: ${swizzle.component}`)
    }
    if (swizzle.sourcePackage !== manifest.source.package || swizzle.sourceVersion !== manifest.source.version || swizzle.license !== manifest.source.license) {
      throw new Error(`Astryx manifest swizzle provenance does not match ${manifest.source.package}: ${swizzle.component}`)
    }
    const wrapper = swizzleWrappers.get(swizzle.component)
    if (wrapper === undefined || wrapper.ownedPath !== swizzle.ownedPath || !wrapper.publicApi.includes(swizzle.publicApi) || wrapper.upstream?.sourcePath !== swizzle.sourcePath || wrapper.upstream?.sourceSha256.toLowerCase() !== swizzle.sourceSha256.toLowerCase() || wrapper.cli?.command !== swizzle.command) {
      throw new Error(`Astryx manifest swizzle must have a matching safeWrapper entry: ${swizzle.component}`)
    }
  }
  for (const component of swizzleWrappers.keys()) {
    if (!legacySwizzles.has(component)) throw new Error(`Astryx safeWrapper swizzle is missing from legacy swizzles: ${component}`)
  }
  for (const entry of manifest.cssImports) {
    assertAppFile(projectRoot, canonicalRoot, entry.importer, "CSS importer")
    if (isAppRelativeCssPath(entry.path) && !isRegularFile(path.join(projectRoot, entry.path))) {
      throw new Error(`Astryx manifest CSS path must be an existing app file: ${entry.path}`)
    }
  }
  for (const entry of manifest.unsafeImportBaseline) {
    assertAppFile(projectRoot, canonicalRoot, entry.importer, "unsafe import baseline importer")
  }
}

function resolveInstalledPackageRoot(projectRoot: string, packageName: string): string {
  const packagePath = path.join(projectRoot, "node_modules", ...packageName.split("/"))
  const packageJsonPath = path.join(packagePath, "package.json")
  if (!isRegularFile(packageJsonPath)) throw new Error(`Unable to validate Astryx package provenance: ${packageJsonPath}`)
  return realpathSync(packagePath)
}

function assertAppFile(projectRoot: string, canonicalRoot: string, relativePath: string, label: string): void {
  const absolutePath = path.resolve(projectRoot, relativePath)
  if (!isRegularFile(absolutePath)) throw new Error(`Astryx manifest ${label} must be an existing regular file: ${relativePath}`)
  const canonicalPath = realpathSync(absolutePath)
  if (canonicalPath !== canonicalRoot && !canonicalPath.startsWith(`${canonicalRoot}${path.sep}`)) {
    throw new Error(`Astryx manifest ${label} must remain under the canonical app root: ${relativePath}`)
  }
}

function validateUpstreamProvenance(
  manifest: UiGuardManifest,
  corePackage: { version?: unknown; license?: unknown; exports?: Record<string, unknown> },
  installedPackageRoot: string,
  upstream: UiGuardUpstream,
): void {
  if (upstream.package !== manifest.source.package || upstream.version !== manifest.source.version || upstream.license !== manifest.source.license) {
    throw new Error(`Astryx safeWrapper upstream metadata does not match ${manifest.source.package}`)
  }
  validateManifestPath(upstream.sourcePath)
  validateModuleSpecifier(upstream.import, "upstream import")
  if (upstream.import !== manifest.source.package && !upstream.import.startsWith(`${manifest.source.package}/`)) {
    throw new Error(`Astryx safeWrapper upstream import is outside ${manifest.source.package}: ${upstream.import}`)
  }
  const exportName = upstream.import === manifest.source.package ? "." : `.${upstream.import.slice(manifest.source.package.length)}`
  const declaredExport = corePackage.exports?.[exportName]
  if (!isRecord(declaredExport) || declaredExport.source !== `./${upstream.sourcePath}`) {
    throw new Error(`Astryx safeWrapper upstream source does not match package.json exports: ${upstream.import}`)
  }
  const sourcePath = path.join(installedPackageRoot, upstream.sourcePath)
  if (!isRegularFile(sourcePath)) throw new Error(`Astryx safeWrapper upstream source is not a regular file: ${upstream.sourcePath}`)
  const packageRoot = realpathSync(installedPackageRoot)
  const canonicalSource = realpathSync(sourcePath)
  if (canonicalSource !== packageRoot && !canonicalSource.startsWith(`${packageRoot}${path.sep}`)) {
    throw new Error(`Astryx safeWrapper upstream source escapes installed package root: ${upstream.sourcePath}`)
  }
  const actualHash = createHash("sha256").update(readFileSync(sourcePath)).digest("hex")
  if (actualHash !== upstream.sourceSha256.toLowerCase()) {
    throw new Error(`Astryx safeWrapper upstream source hash mismatch: ${upstream.import}`)
  }
}

function validateCliProvenance(projectRoot: string, evidence: UiGuardCliEvidence): void {
  if (evidence.package !== "@astryxdesign/cli") throw new Error(`Astryx CLI evidence package is invalid: ${evidence.package}`)
  let appPackage: { dependencies?: Record<string, unknown>; devDependencies?: Record<string, unknown> }
  try {
    appPackage = JSON.parse(readFileSync(path.join(projectRoot, "package.json"), "utf8")) as typeof appPackage
  } catch (error) {
    throw new Error(`Unable to validate Astryx CLI app dependency provenance: ${evidence.package}`, { cause: error })
  }
  const declaredVersion = appPackage.dependencies?.[evidence.package] ?? appPackage.devDependencies?.[evidence.package]
  if (declaredVersion !== evidence.version) throw new Error(`Astryx CLI evidence version does not match package.json: ${evidence.package}`)
  const cliRoot = resolveInstalledPackageRoot(projectRoot, evidence.package)
  let cliPackage: { name?: unknown; version?: unknown }
  try {
    cliPackage = JSON.parse(readFileSync(path.join(cliRoot, "package.json"), "utf8")) as typeof cliPackage
  } catch (error) {
    throw new Error(`Unable to validate Astryx CLI provenance: ${evidence.package}`, { cause: error })
  }
  if (cliPackage.name !== evidence.package || cliPackage.version !== evidence.version) throw new Error(`Astryx CLI evidence metadata does not match installed package: ${evidence.package}`)
  const lockfilePath = path.resolve(projectRoot, "..", "..", "pnpm-lock.yaml")
  const lockfile = readFileSync(lockfilePath, "utf8")
  const lockPattern = new RegExp(`['"]?${escapeRegExp(evidence.package)}@${escapeRegExp(evidence.version)}['"]?:\\n\\s+resolution: \\{integrity: ([^}]+)\\}`)
  if (!lockPattern.test(lockfile)) throw new Error(`Astryx CLI evidence package is missing from pnpm-lock.yaml: ${evidence.package}`)
  if (!/^[a-f0-9]{64}$/i.test(evidence.evidenceSha256)) throw new Error(`Astryx CLI evidence hash is invalid: ${evidence.component}`)
  const actualEvidenceHash = createHash("sha256").update(evidence.evidenceStdout, "utf8").digest("hex")
  if (actualEvidenceHash !== evidence.evidenceSha256.toLowerCase()) throw new Error(`Astryx CLI evidence stdout hash mismatch: ${evidence.component}`)
  let evidenceDocument: unknown
  try {
    evidenceDocument = JSON.parse(evidence.evidenceStdout) as unknown
  } catch (error) {
    throw new Error(`Astryx CLI evidence stdout must be valid JSON: ${evidence.component}`, { cause: error })
  }
  const allowedEvidenceTypes = evidence.command.includes(" swizzle ") ? new Set(["swizzle.detail", "component.detail"]) : new Set(["component.detail"])
  if (!isRecord(evidenceDocument) || typeof evidenceDocument.type !== "string" || !allowedEvidenceTypes.has(evidenceDocument.type)) {
    throw new Error(`Astryx CLI evidence stdout type does not match command: ${evidence.component}`)
  }
  const data = evidenceDocument.data
  const evidenceComponent = isRecord(data) ? data.name ?? data.component : undefined
  if (evidenceComponent !== evidence.component) throw new Error(`Astryx CLI evidence stdout component does not match command: ${evidence.component}`)
}

function validateRootBarrelContract(projectRoot: string, rootBarrel: UiGuardRootBarrel): void {
  readRootBarrelPublicApi(projectRoot, rootBarrel.path)
}

type UiGuardRootBarrelExport = UiGuardPublicSource

function readRootBarrelPublicApi(projectRoot: string, rootBarrelPath: string): { names: readonly string[]; runtimeNames: ReadonlySet<string>; entries: readonly UiGuardRootBarrelExport[] } {
  const source = readFileSync(path.join(projectRoot, rootBarrelPath), "utf8")
  const sourceFile = ts.createSourceFile(rootBarrelPath, source, ts.ScriptTarget.Latest, true, ts.ScriptKind.TS)
  const names: string[] = []
  const runtimeNames = new Set<string>()
  const entries: UiGuardRootBarrelExport[] = []
  const seen = new Set<string>()
  const add = (exported: string, imported: string, sourcePath: string, kind: "runtime" | "type"): void => {
    if (exported === "default" || imported === "default") throw new Error("Astryx root barrel default exports are forbidden")
    if (seen.has(exported)) throw new Error(`Astryx root barrel contains a duplicate export: ${exported}`)
    seen.add(exported)
    names.push(exported)
    if (kind === "runtime") runtimeNames.add(exported)
    entries.push({ exported, imported, sourcePath, kind })
  }
  for (const statement of sourceFile.statements) {
    if (ts.isExportDeclaration(statement)) {
      if (statement.moduleSpecifier === undefined || !ts.isStringLiteral(statement.moduleSpecifier)) throw new Error("Astryx root barrel exports must name an explicit source module")
      const clause = statement.exportClause
      if (clause === undefined || ts.isNamespaceExport(clause)) throw new Error("Astryx root barrel wildcard and namespace exports are forbidden")
      if (!ts.isNamedExports(clause)) throw new Error("Astryx root barrel export form is unsupported")
      const sourcePath = resolveRootExportSourcePath(projectRoot, rootBarrelPath, statement.moduleSpecifier.text)
      for (const element of clause.elements) {
        const imported = element.propertyName?.text ?? element.name.text
        add(element.name.text, imported, sourcePath, statement.isTypeOnly || element.isTypeOnly ? "type" : "runtime")
      }
      continue
    }
    if (ts.isExportAssignment(statement)) throw new Error("Astryx root barrel default exports are forbidden")
    const modifiers = ts.canHaveModifiers(statement) ? ts.getModifiers(statement) : undefined
    if (modifiers?.some((modifier) => modifier.kind === ts.SyntaxKind.ExportKeyword)) throw new Error("Astryx root barrel only allows explicit named exports from source modules")
  }
  return { names, runtimeNames, entries }
}

function resolveRootExportSourcePath(projectRoot: string, rootBarrelPath: string, moduleSpecifier: string): string {
  if (!moduleSpecifier.startsWith(".")) throw new Error(`Astryx root barrel export module must be relative: ${moduleSpecifier}`)
  const rootDirectory = path.dirname(path.join(projectRoot, rootBarrelPath))
  const base = path.resolve(rootDirectory, moduleSpecifier)
  const candidates = [
    base,
    ...[".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs"].map((extension) => `${base}${extension}`),
    ...["index.ts", "index.tsx", "index.js", "index.jsx", "index.mjs", "index.cjs"].map((name) => path.join(base, name)),
  ]
  const sourcePath = candidates.find(isRegularFile)
  if (sourcePath === undefined) throw new Error(`Astryx root barrel export module does not resolve: ${moduleSpecifier}`)
  const canonicalRoot = realpathSync(projectRoot)
  const canonicalPath = realpathSync(sourcePath)
  if (canonicalPath !== canonicalRoot && !canonicalPath.startsWith(`${canonicalRoot}${path.sep}`)) throw new Error(`Astryx root barrel export module escapes app root: ${moduleSpecifier}`)
  return toManifestPath(path.relative(projectRoot, canonicalPath))
}

function sameStringSet(left: readonly string[] | ReadonlySet<string>, right: readonly string[] | ReadonlySet<string>): boolean {
  const leftValues: readonly string[] = Array.isArray(left) ? left : [...left]
  const rightValues = right instanceof Set ? right : new Set(right)
  return leftValues.length === rightValues.size && leftValues.every((value) => rightValues.has(value))
}

function samePublicSourceSet(left: readonly UiGuardPublicSource[], right: readonly UiGuardPublicSource[]): boolean {
  if (left.length !== right.length) return false
  const key = (entry: UiGuardPublicSource): string => JSON.stringify([entry.exported, entry.imported, entry.sourcePath, entry.kind])
  const expected = new Set(right.map(key))
  return left.every((entry) => expected.has(key(entry)))
}

function isOwnedPublicSource(wrapper: UiGuardSafeWrapper, member: UiGuardSafeWrapperMember | UiGuardSafeWrapperComponent, publicSource: UiGuardPublicSource): boolean {
  if (publicSource.sourcePath === wrapper.ownedPath) return true
  if (member.component === "Selector" && publicSource.sourcePath === "src/ui/astryx/selectors/shared.ts") return true
  const navigationComponents = new Set(["SideNav", "SideNavHeading", "SideNavItem", "SideNavSection", "TreeList"])
  return navigationComponents.has(member.component) && publicSource.sourcePath === "src/ui/astryx/navigation/types.ts"
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
    if ("dynamicKind" in candidate && candidate.unknownSpecifier) {
      issues.push(issue("unsafe-direct-import", mode === "enforce" ? "error" : "warning", relativePath, lineForOffset(source, candidate.offset), `Dynamic ${candidate.dynamicKind}() module specifier must be a string literal under the static import policy.`))
      continue
    }
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

type DynamicImport = StaticImport & { readonly dynamicKind: "import" | "require"; readonly unknownSpecifier: boolean }

function collectStaticImports(source: string, relativePath: string): readonly (StaticImport | DynamicImport)[] {
  const extension = path.extname(relativePath).toLowerCase()
  if (extension === ".css") return []
  if (![".js", ".jsx", ".mjs", ".cjs", ".ts", ".tsx"].includes(extension)) return []
  const scriptKind = extension === ".tsx" ? ts.ScriptKind.TSX : extension === ".jsx" ? ts.ScriptKind.JSX : ts.ScriptKind.TS
  const sourceFile = ts.createSourceFile(relativePath, source, ts.ScriptTarget.Latest, true, scriptKind)
  const imports: (StaticImport | DynamicImport)[] = []
  const visit = (node: ts.Node): void => {
    if (ts.isImportDeclaration(node) && ts.isStringLiteral(node.moduleSpecifier)) {
      imports.push({ value: node.moduleSpecifier.text, offset: node.moduleSpecifier.getStart(sourceFile) })
    } else if (ts.isExportDeclaration(node) && node.moduleSpecifier !== undefined && ts.isStringLiteral(node.moduleSpecifier)) {
      imports.push({ value: node.moduleSpecifier.text, offset: node.moduleSpecifier.getStart(sourceFile) })
    } else if (ts.isCallExpression(node)) {
      const isDynamicImport = node.expression.kind === ts.SyntaxKind.ImportKeyword
      const isRequire = ts.isIdentifier(node.expression) && node.expression.text === "require"
      if (isDynamicImport || isRequire) {
        const argument = node.arguments[0]
        if (argument !== undefined && ts.isStringLiteral(argument)) {
          imports.push({ value: argument.text, offset: argument.getStart(sourceFile), dynamicKind: isDynamicImport ? "import" : "require", unknownSpecifier: false })
        } else {
          imports.push({ value: "<dynamic-module-specifier>", offset: argument?.getStart(sourceFile) ?? node.getStart(sourceFile), dynamicKind: isDynamicImport ? "import" : "require", unknownSpecifier: true })
        }
      }
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
  const staticObjectModel = collectStaticObjectBindings(sourceFile)
  const cspSafeDomPropModel = collectCspSafeDomPropModel(sourceFile, relativePath)
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
      if (isIntrinsicJsxTagName(tagName) && ts.isJsxSpreadAttribute(attribute) && !cspSafeDomPropModel.isSafeExpression(attribute.expression) && spreadMayCarryStyle(attribute.expression, staticObjectModel, attribute)) {
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
    if (ts.isCallExpression(node) && isStyleMutationCall(node, staticObjectModel)) {
      addViolation("dom-style", node.expression, "DOM .style mutation is forbidden under the static CSP style policy.")
    }
    if (ts.isCallExpression(node) && isMarkupInjectionCall(node)) {
      addViolation("inline-style", node.expression, "Runtime HTML/style injection is forbidden under the static CSP style policy.")
    }
    if (ts.isCallExpression(node) && isStyleElementFactory(node)) {
      addViolation("inline-style", node.expression, "Runtime style element injection is forbidden under the static CSP style policy.")
    }
    if (ts.isCallExpression(node) && isCreateElementCall(node) && createElementPropsMayCarryStyle(node, staticObjectModel)) {
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

const CSP_SAFE_DOM_PROPS_MODULE = "src/ui/astryx/dom-props"
const CSP_SAFE_DOM_PROPS_HELPER = "pickCspSafeDomProps"

type CspSafeDomPropBindingKind = "helper" | "namespace" | "other"

type CspSafeDomPropBinding = {
  readonly name: string
  readonly declaration: ts.Node
  kind: CspSafeDomPropBindingKind
}

type CspSafeDomPropScope = {
  readonly node: ts.Node
  readonly parent: CspSafeDomPropScope | undefined
  readonly bindings: Map<string, CspSafeDomPropBinding>
}

type CspSafeDomPropModel = {
  readonly isSafeExpression: (expression: ts.Expression) => boolean
}

/**
 * Resolve the one canonical runtime DOM-prop helper without trusting a local
 * function that merely happens to use the same name. The model intentionally
 * understands only a named/namespace import from `src/ui/astryx/dom-props`
 * and an inline direct call at the intrinsic JSX spread site. Results stored in
 * variables are deliberately not trusted: mutation, escaping, and aliases are
 * difficult to prove safe without reimplementing a full data-flow analysis.
 */
function collectCspSafeDomPropModel(sourceFile: ts.SourceFile, relativePath: string): CspSafeDomPropModel {
  const root: CspSafeDomPropScope = {node: sourceFile, parent: undefined, bindings: new Map()}
  const scopeByNode = new Map<ts.Node, CspSafeDomPropScope>([[sourceFile, root]])

  const isScopeNode = (node: ts.Node): boolean => ts.isBlock(node)
    || ts.isCaseBlock(node)
    || ts.isCatchClause(node)
    || ts.isFunctionDeclaration(node)
    || ts.isFunctionExpression(node)
    || ts.isClassExpression(node)
    || ts.isArrowFunction(node)
    || ts.isMethodDeclaration(node)
    || ts.isGetAccessorDeclaration(node)
    || ts.isSetAccessorDeclaration(node)
    || ts.isConstructorDeclaration(node)

  const isFunctionScopeNode = (node: ts.Node): boolean => node === sourceFile
    || ts.isFunctionDeclaration(node)
    || ts.isFunctionExpression(node)
    || ts.isArrowFunction(node)
    || ts.isMethodDeclaration(node)
    || ts.isGetAccessorDeclaration(node)
    || ts.isSetAccessorDeclaration(node)
    || ts.isConstructorDeclaration(node)

  const registerBinding = (
    scope: CspSafeDomPropScope,
    name: string,
    declaration: ts.Node,
    kind: CspSafeDomPropBindingKind,
  ): CspSafeDomPropBinding => {
    const existing = scope.bindings.get(name)
    if (existing !== undefined) {
      existing.kind = "other"
      return existing
    }
    const binding = {name, declaration, kind}
    scope.bindings.set(name, binding)
    return binding
  }

  const scopeFor = (node: ts.Node): CspSafeDomPropScope => {
    let current: ts.Node | undefined = node
    while (current !== undefined) {
      const scope = scopeByNode.get(current)
      if (scope !== undefined) return scope
      current = current.parent
    }
    return root
  }

  const varScopeFor = (scope: CspSafeDomPropScope): CspSafeDomPropScope => {
    let current = scope
    while (!isFunctionScopeNode(current.node) && current.parent !== undefined) current = current.parent
    return current
  }

  const resolveBinding = (identifier: ts.Identifier, useNode: ts.Node): CspSafeDomPropBinding | undefined => {
    let scope: CspSafeDomPropScope | undefined = scopeFor(useNode)
    while (scope !== undefined) {
      const binding = scope.bindings.get(identifier.text)
      if (binding !== undefined) return binding
      scope = scope.parent
    }
    return undefined
  }

  const canonicalModulePath = (specifier: string): string | undefined => {
    if (specifier === "@/ui/astryx/dom-props") return CSP_SAFE_DOM_PROPS_MODULE
    if (!specifier.startsWith(".")) return undefined
    const resolved = path.posix.normalize(path.posix.join(path.posix.dirname(relativePath), specifier))
    const withoutExtension = resolved.replace(/\.(?:[cm]?[jt]sx?)$/, "")
    return withoutExtension === CSP_SAFE_DOM_PROPS_MODULE ? withoutExtension : undefined
  }

  // Imports are hoisted, so register the canonical bindings before resolving
  // variable initializers even when a source file places an import later.
  for (const statement of sourceFile.statements) {
    if (!ts.isImportDeclaration(statement) || !ts.isStringLiteral(statement.moduleSpecifier)) continue
    if (canonicalModulePath(statement.moduleSpecifier.text) === undefined) continue
    const bindings = statement.importClause?.namedBindings
    if (bindings === undefined) continue
    if (ts.isNamespaceImport(bindings)) {
      registerBinding(root, bindings.name.text, bindings.name, "namespace")
      continue
    }
    for (const element of bindings.elements) {
      const importedName = element.propertyName?.text ?? element.name.text
      if (importedName === CSP_SAFE_DOM_PROPS_HELPER) {
        registerBinding(root, element.name.text, element.name, "helper")
      } else {
        registerBinding(root, element.name.text, element.name, "other")
      }
    }
  }

  const unwrap = (expression: ts.Expression): ts.Expression => {
    let candidate = expression
    while (
      ts.isParenthesizedExpression(candidate)
      || ts.isAsExpression(candidate)
      || ts.isTypeAssertionExpression(candidate)
      || ts.isNonNullExpression(candidate)
      || ts.isSatisfiesExpression(candidate)
    ) {
      candidate = candidate.expression
    }
    return candidate
  }

  const isSafeCall = (expression: ts.Expression): boolean => {
    const candidate = unwrap(expression)
    if (!ts.isCallExpression(candidate)) return false
    const callee = unwrap(candidate.expression)
    if (ts.isIdentifier(callee)) {
      return resolveBinding(callee, candidate)?.kind === "helper"
    }
    if (ts.isPropertyAccessExpression(callee) && callee.name.text === CSP_SAFE_DOM_PROPS_HELPER && ts.isIdentifier(callee.expression)) {
      return resolveBinding(callee.expression, candidate)?.kind === "namespace"
    }
    return false
  }

  const visit = (node: ts.Node, parentScope: CspSafeDomPropScope): void => {
    // Function/class declarations bind in their containing scope and can
    // shadow the canonical import with a same-named local implementation.
    if ((ts.isFunctionDeclaration(node) || ts.isClassDeclaration(node) || ts.isEnumDeclaration(node)) && node.name !== undefined) {
      registerBinding(parentScope, node.name.text, node.name, "other")
    }

    let scope = parentScope
    if (node !== sourceFile && isScopeNode(node)) {
      scope = {node, parent: parentScope, bindings: new Map()}
      scopeByNode.set(node, scope)
      if ((ts.isFunctionExpression(node) || ts.isClassExpression(node)) && node.name !== undefined) {
        registerBinding(scope, node.name.text, node.name, "other")
      }
      if (ts.isFunctionDeclaration(node) || ts.isFunctionExpression(node) || ts.isArrowFunction(node) || ts.isMethodDeclaration(node) || ts.isGetAccessorDeclaration(node) || ts.isSetAccessorDeclaration(node) || ts.isConstructorDeclaration(node)) {
        for (const parameter of node.parameters) {
          for (const identifier of identifiersInBindingName(parameter.name)) registerBinding(scope, identifier.text, identifier, "other")
        }
      }
      if (ts.isCatchClause(node) && node.variableDeclaration !== undefined) {
        for (const identifier of identifiersInBindingName(node.variableDeclaration.name)) registerBinding(scope, identifier.text, identifier, "other")
      }
    }

    if (ts.isVariableDeclaration(node)) {
      const declarationList = node.parent
      // TypeScript represents `var` as the absence of the lexical `let`/`const`
      // flags; `NodeFlags.Var` is not a reliable positive bit to test.
      const isVar = ts.isVariableDeclarationList(declarationList)
        && (declarationList.flags & (ts.NodeFlags.Let | ts.NodeFlags.Const)) === 0
      const bindingScope = isVar ? varScopeFor(scope) : scope
      const identifiers = identifiersInBindingName(node.name)
      for (const identifier of identifiers) {
        registerBinding(bindingScope, identifier.text, identifier, "other")
      }
    }

    ts.forEachChild(node, (child) => visit(child, scope))
  }
  visit(sourceFile, root)

  return {
    isSafeExpression(expression) {
      const candidate = unwrap(expression)
      if (isSafeCall(candidate)) return true
      return false
    },
  }
}

type StaticObjectBinding = {
  readonly name: string
  readonly declaration: ts.Node
  readonly initializer: ts.ObjectLiteralExpression | undefined
  readonly isConstObject: boolean
  mutated: boolean
  escaped: boolean
}

type StaticObjectScope = {
  readonly node: ts.Node
  readonly parent: StaticObjectScope | undefined
  readonly bindings: Map<string, StaticObjectBinding>
}

type StaticObjectModel = {
  readonly mayCarryStyle: (expression: ts.Expression | undefined, useNode: ts.Node, seen?: Set<StaticObjectBinding>) => boolean
  readonly bindingFor: (identifier: ts.Identifier, useNode: ts.Node) => StaticObjectBinding | undefined
}

function collectStaticObjectBindings(sourceFile: ts.SourceFile): StaticObjectModel {
  const root: StaticObjectScope = { node: sourceFile, parent: undefined, bindings: new Map() }
  const scopeByNode = new Map<ts.Node, StaticObjectScope>([[sourceFile, root]])

  const isScopeNode = (node: ts.Node): boolean => ts.isBlock(node)
    || ts.isCaseBlock(node)
    || ts.isCatchClause(node)
    || ts.isFunctionDeclaration(node)
    || ts.isFunctionExpression(node)
    || ts.isArrowFunction(node)
    || ts.isMethodDeclaration(node)
    || ts.isGetAccessorDeclaration(node)
    || ts.isSetAccessorDeclaration(node)
    || ts.isConstructorDeclaration(node)

  const registerBinding = (scope: StaticObjectScope, name: string, declaration: ts.Node, initializer: ts.ObjectLiteralExpression | undefined, isConstObject: boolean): void => {
    const existing = scope.bindings.get(name)
    if (existing !== undefined) {
      existing.mutated = true
      existing.escaped = true
      return
    }
    scope.bindings.set(name, { name, declaration, initializer, isConstObject, mutated: false, escaped: false })
  }

  const visitScopes = (node: ts.Node, parentScope: StaticObjectScope): void => {
    let scope = parentScope
    if (node !== sourceFile && isScopeNode(node)) {
      scope = { node, parent: parentScope, bindings: new Map() }
      scopeByNode.set(node, scope)
      if (ts.isFunctionDeclaration(node) || ts.isFunctionExpression(node) || ts.isArrowFunction(node) || ts.isMethodDeclaration(node) || ts.isGetAccessorDeclaration(node) || ts.isSetAccessorDeclaration(node) || ts.isConstructorDeclaration(node)) {
        for (const parameter of node.parameters) {
          for (const identifier of identifiersInBindingName(parameter.name)) registerBinding(scope, identifier.text, identifier, undefined, false)
        }
      }
      if (ts.isCatchClause(node) && node.variableDeclaration !== undefined) {
        for (const identifier of identifiersInBindingName(node.variableDeclaration.name)) registerBinding(scope, identifier.text, identifier, undefined, false)
      }
    }
    if (ts.isVariableDeclaration(node)) {
      const declarationList = node.parent
      const isConst = ts.isVariableDeclarationList(declarationList) && (declarationList.flags & ts.NodeFlags.Const) !== 0
      const initializer = node.initializer !== undefined && ts.isObjectLiteralExpression(node.initializer) ? node.initializer : undefined
      const identifiers = identifiersInBindingName(node.name)
      for (const identifier of identifiers) {
        registerBinding(scope, identifier.text, identifier, identifiers.length === 1 ? initializer : undefined, isConst && identifiers.length === 1 && initializer !== undefined)
      }
    }
    ts.forEachChild(node, (child) => visitScopes(child, scope))
  }
  visitScopes(sourceFile, root)

  const scopeFor = (node: ts.Node): StaticObjectScope => {
    let current: ts.Node | undefined = node
    while (current !== undefined) {
      const scope = scopeByNode.get(current)
      if (scope !== undefined) return scope
      current = current.parent
    }
    return root
  }

  const bindingFor = (identifier: ts.Identifier, useNode: ts.Node): StaticObjectBinding | undefined => {
    let scope: StaticObjectScope | undefined = scopeFor(useNode)
    while (scope !== undefined) {
      const binding = scope.bindings.get(identifier.text)
      if (binding !== undefined) {
        if (binding.declaration.getStart(sourceFile) >= useNode.getStart(sourceFile)) return binding
        return binding
      }
      scope = scope.parent
    }
    return undefined
  }

  const rootIdentifier = (expression: ts.Expression): ts.Identifier | undefined => {
    if (ts.isParenthesizedExpression(expression)) return rootIdentifier(expression.expression)
    if (ts.isPropertyAccessExpression(expression) || ts.isElementAccessExpression(expression)) return rootIdentifier(expression.expression)
    return ts.isIdentifier(expression) ? expression : undefined
  }

  const markMutation = (expression: ts.Expression | undefined, useNode: ts.Node, escaped = false): void => {
    if (expression === undefined) return
    const identifier = rootIdentifier(expression)
    if (identifier === undefined) return
    const binding = bindingFor(identifier, useNode)
    if (binding === undefined) return
    if (escaped) binding.escaped = true
    else binding.mutated = true
  }

  const visitMutations = (node: ts.Node): void => {
    if (ts.isBinaryExpression(node) && isAssignmentOperator(node.operatorToken.kind)) markMutation(node.left, node.left)
    if (ts.isPrefixUnaryExpression(node) || ts.isPostfixUnaryExpression(node)) {
      if (node.operator === ts.SyntaxKind.PlusPlusToken || node.operator === ts.SyntaxKind.MinusMinusToken) markMutation(node.operand, node.operand)
    }
    if (ts.isVariableDeclaration(node) && ts.isIdentifier(node.name) && node.initializer !== undefined && ts.isIdentifier(node.initializer)) {
      markMutation(node.initializer, node.initializer, true)
    }
    if (ts.isCallExpression(node)) {
      const callee = node.expression
      const isObjectAssign = ts.isPropertyAccessExpression(callee) && ts.isIdentifier(callee.expression) && callee.expression.text === "Object" && (callee.name.text === "assign" || callee.name.text === "defineProperty")
      if (isObjectAssign) markMutation(node.arguments[0], node.arguments[0])
      const isCreateElement = isCreateElementCall(node)
      for (const [index, argument] of node.arguments.entries()) {
        if (!(isCreateElement && index === 1)) markMutation(argument, argument, true)
      }
      if (!isObjectAssign) markMutation(callee, callee, true)
    }
    ts.forEachChild(node, visitMutations)
  }
  visitMutations(sourceFile)

  const mayCarryStyle = (expression: ts.Expression | undefined, useNode: ts.Node, seen = new Set<StaticObjectBinding>()): boolean => {
    if (expression === undefined) return true
    if (ts.isParenthesizedExpression(expression)) return mayCarryStyle(expression.expression, useNode, seen)
    if (ts.isIdentifier(expression)) {
      const binding = bindingFor(expression, useNode)
      if (binding === undefined || binding.declaration.getStart(sourceFile) >= useNode.getStart(sourceFile) || !binding.isConstObject || binding.mutated || binding.escaped || binding.initializer === undefined || seen.has(binding)) return true
      seen.add(binding)
      return mayCarryStyle(binding.initializer, binding.declaration, seen)
    }
    if (!ts.isObjectLiteralExpression(expression)) return true
    for (const property of expression.properties) {
      const name = "name" in property && property.name !== undefined ? property.name.getText(sourceFile) : undefined
      if (name === "style" || name === "dangerouslySetInnerHTML") return true
      if (ts.isSpreadAssignment(property) && mayCarryStyle(property.expression, property, new Set(seen))) return true
      if (property.name !== undefined && ts.isComputedPropertyName(property.name)) return true
    }
    return false
  }

  return { mayCarryStyle, bindingFor }
}

function identifiersInBindingName(name: ts.BindingName): readonly ts.Identifier[] {
  if (ts.isIdentifier(name)) return [name]
  const identifiers: ts.Identifier[] = []
  for (const element of name.elements) {
    if (ts.isBindingElement(element)) identifiers.push(...identifiersInBindingName(element.name))
  }
  return identifiers
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

  const namespaceRoot = (expression: ts.Expression): ts.Identifier | undefined => {
    let candidate = expression
    while (isTransparentExpression(candidate)) candidate = candidate.expression
    if (ts.isIdentifier(candidate)) return candidate
    if (ts.isPropertyAccessExpression(candidate) || ts.isElementAccessExpression(candidate)) return namespaceRoot(candidate.expression)
    return undefined
  }

  const isBindingPattern = (name: ts.BindingName): name is ts.BindingPattern => ts.isObjectBindingPattern(name) || ts.isArrayBindingPattern(name)

  const isUnsafeNamespaceExpression = (expression: ts.Expression): boolean => {
    let candidate = expression
    while (isTransparentExpression(candidate)) candidate = candidate.expression
    return (ts.isIdentifier(candidate) && namespaceAliases.has(candidate.text)) || isBareCoreLoaderExpression(candidate)
  }

  const inspectNamespaceBinding = (pattern: ts.BindingPattern, sourceNode: ts.Node): void => {
    if (ts.isArrayBindingPattern(pattern)) {
      reportUnsafeStar(sourceNode)
      return
    }
    for (const element of pattern.elements) {
      if (element.dotDotDotToken !== undefined) {
        reportUnsafeStar(element)
        continue
      }
      const propertyName = element.propertyName
      if (propertyName !== undefined && ts.isComputedPropertyName(propertyName)) {
        reportUnsafeStar(element)
        continue
      }
      const component = propertyName !== undefined
        ? ts.isIdentifier(propertyName) || ts.isStringLiteral(propertyName) ? propertyName.text : undefined
        : ts.isIdentifier(element.name) ? element.name.text : undefined
      if (component === undefined) {
        reportUnsafeStar(element)
        continue
      }
      reportUnsafe(component, "@astryxdesign/core", propertyName ?? element.name)
      if (element.initializer !== undefined) reportUnsafeStar(element)
      if (isBindingPattern(element.name)) inspectNamespaceBinding(element.name, element)
    }
  }

  const registerDirectNamespaceAliases = (): void => {
    const visitDirect = (node: ts.Node): void => {
      if (ts.isImportDeclaration(node) && ts.isStringLiteral(node.moduleSpecifier) && node.moduleSpecifier.text === "@astryxdesign/core") {
        const bindings = node.importClause?.namedBindings
        if (bindings !== undefined && ts.isNamespaceImport(bindings)) namespaceAliases.add(bindings.name.text)
      }
      if (ts.isVariableDeclaration(node) && ts.isIdentifier(node.name) && node.initializer !== undefined && isBareCoreLoaderExpression(node.initializer)) {
        namespaceAliases.add(node.name.text)
      }
      ts.forEachChild(node, visitDirect)
    }
    visitDirect(sourceFile)
  }

  const registerTransitiveNamespaceAliases = (): void => {
    let changed = true
    while (changed) {
      changed = false
      const visitAlias = (node: ts.Node): void => {
        if (ts.isVariableDeclaration(node) && ts.isIdentifier(node.name) && node.initializer !== undefined) {
          let initializer = node.initializer
          while (isTransparentExpression(initializer)) initializer = initializer.expression
          if (ts.isIdentifier(initializer) && namespaceAliases.has(initializer.text) && !namespaceAliases.has(node.name.text)) {
            namespaceAliases.add(node.name.text)
            changed = true
          }
        }
        ts.forEachChild(node, visitAlias)
      }
      visitAlias(sourceFile)
    }
  }

  registerDirectNamespaceAliases()
  registerTransitiveNamespaceAliases()

  const visit = (node: ts.Node): void => {
    if (ts.isVariableDeclaration(node) && ts.isIdentifier(node.name) && node.initializer !== undefined && isBareCoreLoaderExpression(node.initializer)) {
      namespaceAliases.add(node.name.text)
    }
    if (ts.isVariableDeclaration(node) && ts.isBindingName(node.name) && node.initializer !== undefined && isUnsafeNamespaceExpression(node.initializer)) {
      if (isBindingPattern(node.name)) inspectNamespaceBinding(node.name, node.name)
      const statement = node.parent.parent
      if (ts.isVariableStatement(statement) && hasExportModifier(statement)) reportUnsafeStar(node)
    }
    if (ts.isBinaryExpression(node) && isAssignmentOperator(node.operatorToken.kind) && ts.isIdentifier(node.left) && node.right !== undefined && isUnsafeNamespaceExpression(node.right)) {
      namespaceAliases.add(node.left.text)
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
    } else if (ts.isExportDeclaration(node) && node.moduleSpecifier === undefined) {
      const clause = node.exportClause
      if (clause !== undefined && ts.isNamedExports(clause)) for (const element of clause.elements) {
        const local = element.propertyName ?? element.name
        const localName = ts.isIdentifier(local) || ts.isStringLiteral(local) ? local.text : undefined
        if (localName !== undefined && namespaceAliases.has(localName)) reportUnsafeStar(element)
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
    if (ts.isExportAssignment(node) && isUnsafeNamespaceExpression(node.expression)) reportUnsafeStar(node)
    if (ts.isCallExpression(node) && isBareCoreLoader(node)) reportUnsafeStar(node)
    if (ts.isPropertyAccessExpression(node) || ts.isElementAccessExpression(node)) {
      const component = memberName(node)
      const root = namespaceRoot(node.expression)
      if (component !== undefined && root !== undefined && namespaceAliases.has(root.text)) {
        reportUnsafe(component, "@astryxdesign/core", node)
      }
      if (component !== undefined && isBareCoreLoaderExpression(node.expression)) {
        reportUnsafe(component, "@astryxdesign/core", node)
      }
    }
    if (ts.isIdentifier(node) && namespaceAliases.has(node.text) && !isHandledNamespaceReference(node)) {
      reportUnsafeStar(node)
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
  while (isTransparentExpression(candidate)) candidate = candidate.expression
  return ts.isCallExpression(candidate) && isBareCoreLoader(candidate)
}

function isTransparentExpression(expression: ts.Expression): expression is ts.ParenthesizedExpression | ts.AwaitExpression | ts.AsExpression | ts.TypeAssertion | ts.NonNullExpression | ts.SatisfiesExpression {
  return ts.isParenthesizedExpression(expression)
    || ts.isAwaitExpression(expression)
    || ts.isAsExpression(expression)
    || ts.isTypeAssertionExpression(expression)
    || ts.isNonNullExpression(expression)
    || ts.isSatisfiesExpression(expression)
}

function hasExportModifier(node: ts.Node): boolean {
  const modifiers = ts.canHaveModifiers(node) ? ts.getModifiers(node) : undefined
  return modifiers?.some((modifier) => modifier.kind === ts.SyntaxKind.ExportKeyword) ?? false
}

function isHandledNamespaceReference(node: ts.Identifier): boolean {
  const parent = node.parent
  if (parent === undefined) return false
  if (ts.isNamespaceImport(parent) || ts.isNamespaceExport(parent) || ts.isImportSpecifier(parent) || ts.isExportSpecifier(parent)) return true
  if (ts.isPropertyAccessExpression(parent) || ts.isElementAccessExpression(parent)) return parent.expression === node || (ts.isPropertyAccessExpression(parent) && parent.name === node)
  if (ts.isVariableDeclaration(parent)) {
    if (parent.name === node) return true
    if (parent.initializer !== undefined) {
      let candidate = parent.initializer
      while (isTransparentExpression(candidate)) candidate = candidate.expression
      if (candidate === node) return true
    }
  }
  if (ts.isBinaryExpression(parent) && parent.left === node && isAssignmentOperator(parent.operatorToken.kind)) return true
  if (ts.isPropertyAssignment(parent) && parent.name === node) return true
  return false
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

function isStyleMutationCall(node: ts.CallExpression, objectModel: StaticObjectModel): boolean {
  if (ts.isPropertyAccessExpression(node.expression)) {
    if ((node.expression.name.text === "setProperty" || node.expression.name.text === "removeProperty") && expressionContainsStyle(node.expression.expression)) return true
    if (node.expression.name.text === "setAttribute" || node.expression.name.text === "removeAttribute") {
      return node.arguments.length === 0 || !ts.isStringLiteral(node.arguments[0]) || node.arguments[0].text === "style"
    }
    if (node.expression.name.text === "assign" && ts.isIdentifier(node.expression.expression) && node.expression.expression.text === "Object") {
      return node.arguments.length < 2 || expressionContainsStyle(node.arguments[0]) || objectModel.mayCarryStyle(node.arguments[1], node)
    }
  }
  return false
}

function isMarkupInjectionCall(node: ts.CallExpression): boolean {
  return ts.isPropertyAccessExpression(node.expression) && node.expression.name.text === "insertAdjacentHTML"
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

function isIntrinsicJsxTagName(tagName: string): boolean {
  return /^[a-z]/.test(tagName)
}

function spreadMayCarryStyle(expression: ts.Expression, objectModel: StaticObjectModel, useNode: ts.Node): boolean {
  return objectModel.mayCarryStyle(expression, useNode)
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

function createElementPropsMayCarryStyle(node: ts.CallExpression, objectModel: StaticObjectModel): boolean {
  const tag = node.arguments[0]
  const props = node.arguments[1]
  if (tag === undefined || props === undefined) return false
  if (ts.isStringLiteral(tag) && isIntrinsicJsxTagName(tag.text)) return objectModel.mayCarryStyle(props, node)
  if (ts.isObjectLiteralExpression(props)) return objectModel.mayCarryStyle(props, node)
  return ts.isIdentifier(props) && /style|props/i.test(props.text)
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
  const candidate = normalizeGuardImport(imported)
  if (candidate === undefined) return true
  const segments = candidate.split("/").filter(Boolean)
  return forbiddenRoots.some((root) => {
    const rootSegments = root.split("/").filter(Boolean)
    return rootSegments.length > 0 && segments.some((_, index) => rootSegments.every((segment, offset) => segments[index + offset] === segment))
  })
}

function normalizeGuardImport(imported: string): string | undefined {
  let decoded = imported
  for (let attempt = 0; attempt < 4; attempt += 1) {
    let next: string
    try {
      next = decodeURIComponent(decoded)
    } catch {
      return undefined
    }
    if (next === decoded) break
    decoded = next
    if (attempt === 3) {
      try {
        if (decodeURIComponent(decoded) !== decoded) return undefined
      } catch {
        return undefined
      }
    }
  }
  const candidate = decoded.replaceAll("\\", "/").replace(/^\.\/?/, "")
  if (candidate.length === 0) return undefined
  const normalized = path.posix.normalize(candidate)
  if (normalized === ".") return undefined
  return normalized
}

function sameOrDescendant(candidate: string, root: string): boolean {
  return candidate === root || candidate.startsWith(`${root}/`)
}

function toManifestPath(value: string): string {
  return value.replaceAll(path.sep, "/").replace(/^\.\//, "")
}

function validateManifestPath(value: string): void {
  if (!isNonEmptyString(value) || path.posix.isAbsolute(value) || /^[A-Za-z]:/.test(value) || value.includes("\\")) {
    throw new Error(`Invalid Astryx UI safety manifest path: ${String(value)}`)
  }
  const decoded = decodeManifestPath(value)
  const segments = value.split("/")
  if (decoded !== value || segments.some((segment) => segment.length === 0 || segment === "." || segment === "..") || path.posix.normalize(value) !== value) {
    throw new Error(`Invalid Astryx UI safety manifest path: ${String(value)}`)
  }
}

function validateCssImportPath(value: string): void {
  if (!isNonEmptyString(value) || value.startsWith("/") || value.includes("\\") || value.split("/").includes("..")) {
    throw new Error(`Invalid Astryx UI safety manifest CSS path: ${String(value)}`)
  }
  const clean = value.split(/[?#]/, 1)[0]
  if (clean.startsWith(".")) validateManifestPath(clean)
  else if (decodeManifestPath(clean) !== clean || clean.split("/").some((segment) => segment === "" || segment === "." || segment === "..")) {
    throw new Error(`Invalid Astryx UI safety manifest CSS path: ${String(value)}`)
  }
}

function validateModuleSpecifier(value: string, label: string): void {
  if (!isNonEmptyString(value) || value.includes("\\") || value.startsWith("/") || decodeManifestPath(value) !== value || value.split("/").some((segment) => segment === "" || segment === "." || segment === "..")) {
    throw new Error(`Invalid Astryx UI safety manifest ${label}: ${String(value)}`)
  }
}

function decodeManifestPath(value: string): string {
  let decoded = value
  for (let attempt = 0; attempt < 4; attempt += 1) {
    let next: string
    try {
      next = decodeURIComponent(decoded)
    } catch {
      throw new Error(`Invalid Astryx UI safety manifest path: ${String(value)}`)
    }
    if (next === decoded) return decoded
    decoded = next
  }
  return decoded
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

function assertExactKeys(value: Record<string, unknown>, required: readonly string[], label: string): void {
  const requiredSet = new Set(required)
  for (const key of required) {
    if (!(key in value)) throw new Error(`${label}.${key} is required`)
  }
  for (const key of Object.keys(value)) {
    if (!requiredSet.has(key)) throw new Error(`${label}.${key} is not allowed`)
  }
}

function assertUniqueStrings(values: readonly string[], label: string): void {
  if (new Set(values).size !== values.length) throw new Error(`Astryx UI safety manifest ${label} contains duplicates`)
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === "string" && value.trim().length > 0
}

function isPackageName(value: unknown): value is string {
  return typeof value === "string" && /^(?:@[A-Za-z0-9._~-]+\/)?[A-Za-z0-9._~-]+$/.test(value)
}

function isStringArray(value: unknown): value is readonly string[] {
  return Array.isArray(value) && value.every((entry) => isNonEmptyString(entry))
}

function isSwizzle(value: unknown): value is UiGuardSwizzle {
  if (!isRecord(value)) return false
  try {
    assertExactKeys(value, ["component", "ownedPath", "sourcePackage", "sourceVersion", "sourcePath", "command", "sourceSha256", "reason", "publicApi", "license", "runtimeStylePolicy"], "Astryx UI safety manifest swizzle")
  } catch {
    return false
  }
  return ["component", "ownedPath", "sourcePackage", "sourceVersion", "sourcePath", "command", "sourceSha256", "reason", "publicApi", "license", "runtimeStylePolicy"].every((key) => isNonEmptyString(value[key]))
}

function isResidualCss(value: unknown): value is UiGuardResidualCss {
  if (!isRecord(value)) return false
  try { assertExactKeys(value, ["path", "maxLoc", "owner", "reason", "exitCondition"], "Astryx UI safety manifest residualCss") } catch { return false }
  return isNonEmptyString(value.path) && isSafeNonNegativeInteger(value.maxLoc) && isNonEmptyString(value.owner) && isNonEmptyString(value.reason) && isNonEmptyString(value.exitCondition)
}

function isLegacyRawLayout(value: unknown): value is UiGuardLegacyRawLayout {
  if (!isRecord(value)) return false
  try { assertExactKeys(value, ["path", "maxCount", "owner", "exitCondition"], "Astryx UI safety manifest legacyRawLayout") } catch { return false }
  return isNonEmptyString(value.path) && isSafeNonNegativeInteger(value.maxCount) && isNonEmptyString(value.owner) && isNonEmptyString(value.exitCondition)
}

function isCssImport(value: unknown): value is UiGuardCssImport {
  if (!isRecord(value)) return false
  try { assertExactKeys(value, ["importer", "path"], "Astryx UI safety manifest cssImports") } catch { return false }
  return isNonEmptyString(value.importer) && isNonEmptyString(value.path)
}

function isUnsafeImportBaseline(value: unknown): value is UiGuardUnsafeImportBaseline {
  if (!isRecord(value)) return false
  try { assertExactKeys(value, ["importer", "path"], "Astryx UI safety manifest unsafeImportBaseline") } catch { return false }
  return isNonEmptyString(value.importer) && isNonEmptyString(value.path)
}

function isPageEntrypoint(value: unknown): value is UiGuardPageEntrypoint {
  if (!isRecord(value)) return false
  try { assertExactKeys(value, ["path", "import"], "Astryx UI safety manifest pageContract entrypoint") } catch { return false }
  return isNonEmptyString(value.path) && isNonEmptyString(value.import)
}

function isSafeNonNegativeInteger(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0
}

function isSafeWrapper(value: unknown): value is UiGuardSafeWrapper {
  if (!isRecord(value) || (value.kind !== "component" && value.kind !== "group")) return false
  const base = ["kind", "component", "ownedPath", "origin", "reason", "runtimeStylePolicy"]
  try {
    if (value.kind === "component") {
      assertExactKeys(value, [...base, "publicApi", "publicSources", ...(value.upstream === undefined ? [] : ["upstream"]), ...(value.cli === undefined ? [] : ["cli"])], "Astryx UI safety manifest safeWrappers component")
      if (!isNonEmptyString(value.component) || !isNonEmptyString(value.ownedPath) || !isManifestOrigin(value.origin) || !isNonEmptyString(value.reason) || !isNonEmptyString(value.runtimeStylePolicy) || !isPublicApi(value.publicApi) || !isPublicSources(value.publicSources) || (value.upstream !== undefined && !isUpstream(value.upstream)) || (value.cli !== undefined && !isCliEvidence(value.cli))) return false
      validateSafeWrapperEvidence(value.origin, value.component, value as unknown as UiGuardSafeWrapperComponent)
      return true
    }
    assertExactKeys(value, [...base, "members"], "Astryx UI safety manifest safeWrappers group")
    if (!isNonEmptyString(value.component) || !isNonEmptyString(value.ownedPath) || !isManifestOrigin(value.origin) || !isNonEmptyString(value.reason) || !isNonEmptyString(value.runtimeStylePolicy) || !Array.isArray(value.members) || value.members.length === 0 || !value.members.every(isSafeWrapperMember)) return false
    for (const member of value.members) validateSafeWrapperEvidence(value.origin, member.component, member)
    return true
  } catch {
    return false
  }
}

function assertSafeWrapperUniqueness(wrappers: readonly UiGuardSafeWrapper[]): void {
  const components = new Set<string>()
  const publicApi = new Set<string>()
  const ownedPaths = new Set<string>()
  for (const wrapper of wrappers) {
    if (components.has(wrapper.component)) throw new Error(`Astryx manifest safeWrappers contains a duplicate component: ${wrapper.component}`)
    if (wrapper.kind === "group") components.add(wrapper.component)
    if (ownedPaths.has(wrapper.ownedPath)) throw new Error(`Astryx manifest safeWrappers contains a duplicate ownedPath: ${wrapper.ownedPath}`)
    ownedPaths.add(wrapper.ownedPath)
    const members = wrapper.kind === "group" ? wrapper.members : [wrapper]
    for (const member of members) {
      if (components.has(member.component)) throw new Error(`Astryx manifest safeWrappers contains a duplicate component: ${member.component}`)
      components.add(member.component)
      for (const symbol of member.publicApi) {
        if (publicApi.has(symbol)) throw new Error(`Astryx manifest safeWrappers contains a duplicate publicApi symbol: ${symbol}`)
        publicApi.add(symbol)
      }
    }
  }
}

function isSafeWrapperMember(value: unknown): value is UiGuardSafeWrapperMember {
  if (!isRecord(value)) return false
  try {
  assertExactKeys(value, ["component", "publicApi", "publicSources", ...(value.upstream === undefined ? [] : ["upstream"]), ...(value.cli === undefined ? [] : ["cli"])], "Astryx UI safety manifest safeWrappers member")
  } catch {
    return false
  }
  return isNonEmptyString(value.component) && isPublicApi(value.publicApi) && isPublicSources(value.publicSources) && (value.upstream === undefined || isUpstream(value.upstream)) && (value.cli === undefined || isCliEvidence(value.cli))
}

function validateSafeWrapperEvidence(origin: UiGuardManifestOrigin, component: string, value: UiGuardSafeWrapperComponent | UiGuardSafeWrapperMember): void {
  const hasUpstream = value.upstream !== undefined
  const hasCli = value.cli !== undefined
  if (origin === "app-owned" && (hasUpstream || hasCli)) throw new Error(`app-owned Astryx safe wrapper must not carry upstream or CLI evidence: ${component}`)
  if (origin === "astryx-reimplementation" && (!hasUpstream || !hasCli)) throw new Error(`astryx-reimplementation requires upstream and component CLI evidence: ${component}`)
  if (origin === "astryx-facade" && (!hasUpstream || hasCli)) throw new Error(`astryx-facade requires upstream evidence and forbids CLI evidence: ${component}`)
  if (origin === "astryx-swizzle" && (!hasUpstream || !hasCli)) throw new Error(`astryx-swizzle requires upstream and swizzle CLI evidence: ${component}`)
  if (value.cli !== undefined) {
    if (value.cli.component !== component) throw new Error(`Astryx CLI evidence component mismatch: ${component}`)
    const commandKind = origin === "astryx-swizzle" ? "swizzle" : "component"
    const expected = `pnpm exec astryx --json ${commandKind} ${component}`
    if (value.cli.command !== expected) throw new Error(`Astryx CLI evidence command mismatch for ${component}: expected ${expected}`)
  }
}

function isManifestOrigin(value: unknown): value is UiGuardManifestOrigin {
  return value === "app-owned" || value === "astryx-reimplementation" || value === "astryx-facade" || value === "astryx-swizzle"
}

function isPublicApi(value: unknown): value is readonly string[] {
  return isStringArray(value) && value.length > 0 && value.every((entry) => /^[A-Za-z_$][A-Za-z0-9_$]*$/.test(entry))
}

function isPublicSources(value: unknown): value is readonly UiGuardPublicSource[] {
  return Array.isArray(value) && value.length > 0 && value.every((entry) => {
    if (!isRecord(entry)) return false
    try { assertExactKeys(entry, ["exported", "imported", "sourcePath", "kind"], "Astryx UI safety manifest public source") } catch { return false }
    return isNonEmptyString(entry.exported) && /^[A-Za-z_$][A-Za-z0-9_$]*$/.test(entry.exported)
      && isNonEmptyString(entry.imported) && /^[A-Za-z_$][A-Za-z0-9_$]*$/.test(entry.imported)
      && isNonEmptyString(entry.sourcePath) && (entry.kind === "runtime" || entry.kind === "type")
  })
}

function isUpstream(value: unknown): value is UiGuardUpstream {
  if (!isRecord(value)) return false
  try {
    assertExactKeys(value, ["import", "package", "version", "license", "sourcePath", "sourceSha256"], "Astryx UI safety manifest upstream")
  } catch {
    return false
  }
  return isNonEmptyString(value.import) && isPackageName(value.package) && isNonEmptyString(value.version) && isNonEmptyString(value.license) && isNonEmptyString(value.sourcePath) && isNonEmptyString(value.sourceSha256) && /^[a-f0-9]{64}$/i.test(value.sourceSha256)
}

function isCliEvidence(value: unknown): value is UiGuardCliEvidence {
  if (!isRecord(value)) return false
  try {
    assertExactKeys(value, ["package", "version", "command", "component", "evidenceStdout", "evidenceSha256"], "Astryx UI safety manifest CLI evidence")
  } catch {
    return false
  }
  return isPackageName(value.package) && isNonEmptyString(value.version) && isNonEmptyString(value.command) && isNonEmptyString(value.component) && isNonEmptyString(value.evidenceStdout) && isNonEmptyString(value.evidenceSha256) && /^[a-f0-9]{64}$/i.test(value.evidenceSha256)
}
