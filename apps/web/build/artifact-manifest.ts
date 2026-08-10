import { createHash } from "node:crypto"
import { readdir, readFile, writeFile } from "node:fs/promises"
import { join, relative, sep } from "node:path"

import type { Plugin } from "vite"

export const WEB_ARTIFACT_FORMAT_VERSION = 1
export const WEB_ARTIFACT_BASE_PATH = "/app/"
export const WEB_ARTIFACT_ENTRYPOINT = "index.html"
export const WEB_ARTIFACT_MANIFEST_PATH = "manifest.json"
export const WEB_PROTOCOL_VERSION = "v1"

const BUILD_ID_DOMAIN = "kanban-tool:web-artifact-build-id:v1"
const BUILD_ID_PREFIX = "sha256:"
const SHA256_HEX_LENGTH = 64

export type WebArtifactSource = string | Uint8Array

export type WebArtifactOutput = {
  path: string
  source: WebArtifactSource
}

export type WebArtifactFile = {
  path: string
  bytes: number
  sha256: string
}

export type WebArtifactManifest = {
  formatVersion: number
  basePath: string
  entrypoint: string
  serverVersion: string
  protocolVersion: string
  buildId: string
  files: WebArtifactFile[]
}

export type WebArtifactBuildInput = {
  formatVersion?: number
  basePath?: string
  entrypoint?: string
  serverVersion: string
  protocolVersion?: string
  files: readonly WebArtifactFile[]
}

export class WebArtifactManifestError extends Error {
  constructor(message: string) {
    super(message)
    this.name = "WebArtifactManifestError"
  }
}

export type BuildWebArtifactManifestInput = {
  serverVersion: string
  protocolVersion?: string
  outputs: readonly WebArtifactOutput[]
}

/** Build the canonical static manifest from the final Vite output inventory. */
export function buildWebArtifactManifest({
  serverVersion,
  protocolVersion = WEB_PROTOCOL_VERSION,
  outputs,
}: BuildWebArtifactManifestInput): WebArtifactManifest {
  if (outputs.length === 0) {
    throw new WebArtifactManifestError("Web artifact output inventory must not be empty")
  }

  const files = outputs.map(({ path, source }) => {
    validateArtifactPath(path)
    const bytes = toBytes(source)
    return {
      path,
      bytes: bytes.byteLength,
      sha256: `${BUILD_ID_PREFIX}${createHash("sha256").update(bytes).digest("hex")}`,
    }
  })

  files.sort(compareArtifactFiles)
  validateArtifactFiles(files)

  const buildInput: WebArtifactBuildInput = {
    formatVersion: WEB_ARTIFACT_FORMAT_VERSION,
    basePath: WEB_ARTIFACT_BASE_PATH,
    entrypoint: WEB_ARTIFACT_ENTRYPOINT,
    serverVersion,
    protocolVersion,
    files,
  }

  return {
    formatVersion: WEB_ARTIFACT_FORMAT_VERSION,
    basePath: WEB_ARTIFACT_BASE_PATH,
    entrypoint: WEB_ARTIFACT_ENTRYPOINT,
    serverVersion,
    protocolVersion,
    buildId: webArtifactBuildIdFor(buildInput),
    files,
  }
}

/** Calculate the canonical build ID for the Rust/TypeScript shared preimage. */
export function webArtifactBuildIdFor(input: WebArtifactBuildInput): string {
  const digest = createHash("sha256").update(webArtifactBuildPreimage(input)).digest("hex")
  return `${BUILD_ID_PREFIX}${digest}`
}

/** Expose the canonical framed preimage for cross-language contract tests. */
export function webArtifactBuildPreimage(input: WebArtifactBuildInput): Buffer {
  const formatVersion = input.formatVersion ?? WEB_ARTIFACT_FORMAT_VERSION
  const basePath = input.basePath ?? WEB_ARTIFACT_BASE_PATH
  const entrypoint = input.entrypoint ?? WEB_ARTIFACT_ENTRYPOINT
  const protocolVersion = input.protocolVersion ?? WEB_PROTOCOL_VERSION

  validateBuildInputs({
    ...input,
    formatVersion,
    basePath,
    entrypoint,
    protocolVersion,
  })

  const parts: Buffer[] = [
    frameText(BUILD_ID_DOMAIN),
    frameU64(formatVersion),
    frameText(basePath),
    frameText(entrypoint),
    frameText(input.serverVersion),
    frameText(protocolVersion),
    frameU64(input.files.length),
  ]

  for (const file of input.files) {
    parts.push(frameText(file.path), frameU64(file.bytes), decodeSha256(file.sha256, "file.sha256"))
  }

  return Buffer.concat(parts)
}

/** Validate a complete manifest and its build ID. */
export function validateWebArtifactManifest(manifest: WebArtifactManifest): void {
  const input: WebArtifactBuildInput = {
    formatVersion: manifest.formatVersion,
    basePath: manifest.basePath,
    entrypoint: manifest.entrypoint,
    serverVersion: manifest.serverVersion,
    protocolVersion: manifest.protocolVersion,
    files: manifest.files,
  }
  validateBuildInputs(input)
  validateBuildId(manifest.buildId, "buildId")
  const expected = webArtifactBuildIdFor(input)
  if (manifest.buildId !== expected) {
    throw new WebArtifactManifestError(
      `Web artifact buildId does not match manifest=${manifest.buildId}, expected=${expected}`,
    )
  }
}

/** Emit `manifest.json` after Vite has produced its final output bundle. */
export function createWebArtifactManifestPlugin({
  serverVersion,
  protocolVersion = WEB_PROTOCOL_VERSION,
}: {
  serverVersion: string
  protocolVersion?: string
}): Plugin {
  let resolvedOutputDirectory: string | undefined

  return {
    name: "web-artifact-manifest",
    apply: "build",
    enforce: "post",
    configResolved(config) {
      resolvedOutputDirectory = config.build.outDir
    },
    generateBundle(_options, bundle) {
      const outputs: WebArtifactOutput[] = Object.values(bundle)
        .filter(({ fileName }) => fileName !== WEB_ARTIFACT_MANIFEST_PATH)
        .map((output) => ({
          path: output.fileName,
          source: output.type === "asset" ? output.source : output.code,
        }))
      // Rolldown performs a post-generateBundle rewrite (for example replacing
      // `__VITE_PRELOAD__` in dynamic imports), so this hook only validates the
      // final bundle shape. The canonical manifest is emitted once from the
      // final on-disk bytes in writeBundle below.
      buildWebArtifactManifest({ serverVersion, protocolVersion, outputs })
    },
    async writeBundle(options) {
      const outputDirectory = resolvedOutputDirectory ?? options.dir
      if (!outputDirectory) {
        throw new WebArtifactManifestError("Web artifact output directory is unavailable")
      }
      const outputs = await collectOutputFiles(outputDirectory)
      const manifest = buildWebArtifactManifest({ serverVersion, protocolVersion, outputs })
      await writeFile(
        join(outputDirectory, WEB_ARTIFACT_MANIFEST_PATH),
        `${JSON.stringify(manifest, null, 2)}\n`,
        "utf8",
      )
    },
  }
}

async function collectOutputFiles(outputDirectory: string): Promise<WebArtifactOutput[]> {
  const outputs: WebArtifactOutput[] = []

  async function visit(directory: string): Promise<void> {
    for (const entry of await readdir(directory, { withFileTypes: true })) {
      const absolutePath = join(directory, entry.name)
      if (entry.name === WEB_ARTIFACT_MANIFEST_PATH && directory === outputDirectory) continue
      if (entry.isDirectory()) {
        await visit(absolutePath)
      } else if (entry.isFile()) {
        outputs.push({
          path: relative(outputDirectory, absolutePath).split(sep).join("/"),
          source: await readFile(absolutePath),
        })
      } else {
        throw new WebArtifactManifestError(
          `Web artifact output contains unsupported filesystem entry: ${absolutePath}`,
        )
      }
    }
  }

  await visit(outputDirectory)
  return outputs
}

function validateBuildInputs(input: WebArtifactBuildInput): void {
  if (input.formatVersion !== WEB_ARTIFACT_FORMAT_VERSION) {
    throw new WebArtifactManifestError(
      `Unsupported Web artifact formatVersion: ${input.formatVersion}`,
    )
  }
  if (input.basePath !== WEB_ARTIFACT_BASE_PATH) {
    throw new WebArtifactManifestError(`Web artifact basePath must be ${WEB_ARTIFACT_BASE_PATH}`)
  }
  if (input.entrypoint !== WEB_ARTIFACT_ENTRYPOINT) {
    throw new WebArtifactManifestError(`Web artifact entrypoint must be ${WEB_ARTIFACT_ENTRYPOINT}`)
  }
  validateServerVersion(input.serverVersion)
  if (input.protocolVersion !== WEB_PROTOCOL_VERSION) {
    throw new WebArtifactManifestError(
      `protocolVersion must be exactly ${WEB_PROTOCOL_VERSION} (API wire generation)`,
    )
  }
  validateArtifactFiles(input.files)
}

function validateServerVersion(value: string): void {
  if (
    value.length === 0 ||
    value.trim() !== value ||
    !/^[0-9]+\.[0-9]+\.[0-9]+$/.test(value) ||
    value.split(".").some((part) => part.length > 1 && part.startsWith("0"))
  ) {
    throw new WebArtifactManifestError("serverVersion must be numeric major.minor.patch")
  }
}

function validateArtifactFiles(files: readonly WebArtifactFile[]): void {
  if (files.length === 0) {
    throw new WebArtifactManifestError("Web artifact files must not be empty")
  }

  let previous: Buffer | undefined
  let hasEntrypoint = false
  for (const file of files) {
    validateArtifactPath(file.path)
    if (file.path === WEB_ARTIFACT_ENTRYPOINT) hasEntrypoint = true
    if (!Number.isSafeInteger(file.bytes) || file.bytes < 0) {
      throw new WebArtifactManifestError(`Invalid byte count for ${file.path}`)
    }
    decodeSha256(file.sha256, "file.sha256")

    const current = Buffer.from(file.path, "utf8")
    if (previous) {
      const order = Buffer.compare(previous, current)
      if (order === 0) {
        throw new WebArtifactManifestError(`Duplicate Web artifact path: ${file.path}`)
      }
      if (order > 0) {
        throw new WebArtifactManifestError("Web artifact files must be UTF-8 byte sorted")
      }
    }
    previous = current
  }

  if (!hasEntrypoint) {
    throw new WebArtifactManifestError(`Web artifact files must include ${WEB_ARTIFACT_ENTRYPOINT}`)
  }
}

function validateArtifactPath(path: string): void {
  if (
    path.length === 0 ||
    path.startsWith("/") ||
    path.includes("\\") ||
    path === WEB_ARTIFACT_MANIFEST_PATH ||
    !/^[A-Za-z0-9._/-]+$/.test(path)
  ) {
    throw new WebArtifactManifestError(`Invalid Web artifact path: ${JSON.stringify(path)}`)
  }
  if (path.split("/").some((segment) => segment.length === 0 || segment === "." || segment === "..")) {
    throw new WebArtifactManifestError(`Web artifact path has unsafe segment: ${JSON.stringify(path)}`)
  }
}

function validateBuildId(value: string, field: string): void {
  decodeSha256(value, field)
}

function decodeSha256(value: string, field: string): Buffer {
  if (
    !value.startsWith(BUILD_ID_PREFIX) ||
    value.length !== BUILD_ID_PREFIX.length + SHA256_HEX_LENGTH ||
    !/^[0-9a-f]{64}$/.test(value.slice(BUILD_ID_PREFIX.length))
  ) {
    throw new WebArtifactManifestError(`${field} must be sha256:<64 lowercase hex>`)
  }
  return Buffer.from(value.slice(BUILD_ID_PREFIX.length), "hex")
}

function frameU64(value: number | bigint): Buffer {
  const numeric = typeof value === "bigint" ? value : BigInt(value)
  if (numeric < 0n || numeric > 0xffff_ffff_ffff_ffffn) {
    throw new WebArtifactManifestError("u64 value is out of range")
  }
  const frame = Buffer.alloc(8)
  frame.writeBigUInt64BE(numeric)
  return frame
}

function frameText(value: string): Buffer {
  const bytes = Buffer.from(value, "utf8")
  return Buffer.concat([frameU64(bytes.byteLength), bytes])
}

function toBytes(source: WebArtifactSource): Buffer {
  return typeof source === "string" ? Buffer.from(source, "utf8") : Buffer.from(source)
}

function compareArtifactFiles(left: WebArtifactFile, right: WebArtifactFile): number {
  return Buffer.compare(Buffer.from(left.path, "utf8"), Buffer.from(right.path, "utf8"))
}
