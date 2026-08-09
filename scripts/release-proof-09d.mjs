#!/usr/bin/env node

import { brotliCompressSync, constants as zlibConstants } from "node:zlib"
import { constants as fsConstants } from "node:fs"
import { lstat, open, readFile, rename, rm } from "node:fs/promises"
import path from "node:path"

function usage() {
  throw new Error("usage: release-proof-09d.mjs artifact --root <workspace> | write-json --path <file> | self-test")
}

function parseRoot(argv) {
  if (argv[0] !== "artifact" || argv[1] !== "--root" || !argv[2] || argv.length !== 3) usage()
  return path.resolve(argv[2])
}

function normalizeReference(raw, importer) {
  const value = raw.split("?", 1)[0].split("#", 1)[0]
  if (value === "" || value.startsWith("#") || value.startsWith("data:") || value.startsWith("http:") || value.startsWith("https:") || value.startsWith("//")) return null
  const withoutBase = value.startsWith("/app/") ? value.slice("/app/".length) : value.replace(/^\//, "")
  if (withoutBase === value && !value.startsWith("/")) {
    const resolved = path.posix.normalize(path.posix.join(path.posix.dirname(importer), value))
    if (resolved.startsWith("../") || resolved === "..") throw new Error(`initial artifact dependency escapes dist: ${raw} (referenced by ${importer})`)
    return resolved
  }
  const resolved = path.posix.normalize(withoutBase)
  if (resolved.startsWith("../") || resolved === "..") throw new Error(`initial artifact dependency escapes dist: ${raw} (referenced by ${importer})`)
  return resolved
}

function referencesFor(file, text) {
  const references = []
  if (file === "index.html") {
    for (const match of text.matchAll(/(?:src|href)=["']([^"']+)["']/g)) references.push(match[1])
  }
  if (file.endsWith(".css")) {
    for (const match of text.matchAll(/url\(\s*["']?([^\)"']+)["']?\s*\)/g)) references.push(match[1])
    for (const match of text.matchAll(/@import\s+["']([^"']+)["']/g)) references.push(match[1])
  }
  if (file.endsWith(".js") || file.endsWith(".mjs")) {
    for (const match of text.matchAll(/\bimport\s*(?!\()(?:(?:[^"'();]*?)\s*from\s*)?["']([^"']+)["']/g)) references.push(match[1])
  }
  return references
}

function selfTest() {
  const javascript = [
    'import{entry}from"./entry.js";',
    'import "./side-effect.js";',
    'const lazy = import("./lazy.js");',
  ].join("\n")
  const javascriptReferences = referencesFor("assets/main.js", javascript)
  if (JSON.stringify(javascriptReferences) !== JSON.stringify(["./entry.js", "./side-effect.js"])) {
    throw new Error(`static import parser self-test failed: ${JSON.stringify(javascriptReferences)}`)
  }
  const cssReferences = referencesFor("assets/main.css", '@import url("./theme.css"); body { background: url(../img.svg); }')
  if (JSON.stringify(cssReferences) !== JSON.stringify(["./theme.css", "../img.svg"])) {
    throw new Error(`CSS dependency parser self-test failed: ${JSON.stringify(cssReferences)}`)
  }
  let traversalRejected = false
  try { normalizeReference("../../outside.js", "assets/main.js") } catch { traversalRejected = true }
  if (!traversalRejected) throw new Error("dependency traversal self-test failed")
  const missingReference = normalizeReference("./missing.js", "assets/main.js")
  if (missingReference !== "assets/missing.js") throw new Error(`unexpected normalized missing dependency: ${missingReference}`)
  const missingRejected = !new Set(["assets/main.js"]).has(missingReference)
  if (!missingRejected) throw new Error("missing manifest dependency self-test failed")
  process.stdout.write("release-proof-09d helper self-test passed\n")
}

async function atomicWrite(destination, content) {
  const target = path.resolve(destination)
  const parent = path.dirname(target)
  const temporary = path.join(parent, `.${path.basename(target)}.${process.pid}.${Date.now()}.tmp`)
  try {
    try {
      const existing = await lstat(target)
      if (existing.isSymbolicLink() || !existing.isFile() || existing.nlink !== 1) {
        throw new Error(`atomic output destination is not a regular single-link file: ${target}`)
      }
    } catch (error) {
      if (error?.code !== "ENOENT") throw error
    }
    const handle = await open(
      temporary,
      fsConstants.O_WRONLY | fsConstants.O_CREAT | fsConstants.O_EXCL | fsConstants.O_NOFOLLOW,
      0o600,
    )
    try {
      const stat = await handle.stat()
      if (!stat.isFile() || stat.nlink !== 1) throw new Error(`atomic output temporary is not a regular single-link file: ${temporary}`)
      await handle.writeFile(content)
      await handle.sync()
    } finally {
      await handle.close().catch(() => undefined)
    }
    await rename(temporary, target)
    const written = await lstat(target)
    if (written.isSymbolicLink() || !written.isFile() || written.nlink !== 1) {
      throw new Error(`atomic output destination changed to an unsafe file: ${target}`)
    }
  } catch (error) {
    await rm(temporary, { force: true }).catch(() => undefined)
    throw error
  }
}

async function writeStdin(argv) {
  if (argv[0] !== "write-json" || argv[1] !== "--path" || !argv[2] || argv.length !== 3) usage()
  const chunks = []
  for await (const chunk of process.stdin) chunks.push(Buffer.from(chunk))
  await atomicWrite(argv[2], Buffer.concat(chunks))
}

async function artifact(root) {
  const dist = path.join(root, "apps/web/dist")
  const manifest = JSON.parse(await readFile(path.join(dist, "manifest.json"), "utf8"))
  const descriptors = new Map(manifest.files.map((file) => [file.path, file]))
  if (!descriptors.has("index.html")) throw new Error("artifact manifest does not contain index.html")
  const queue = ["index.html"]
  const selected = new Set()
  const bytesByPath = new Map()
  while (queue.length > 0) {
    const current = queue.shift()
    if (selected.has(current)) continue
    const descriptor = descriptors.get(current)
    if (!descriptor) throw new Error(`initial artifact dependency is absent from manifest: ${current}`)
    const bytes = await readFile(path.join(dist, current))
    selected.add(current)
    bytesByPath.set(current, bytes)
    const text = bytes.toString("utf8")
    for (const raw of referencesFor(current, text)) {
      const reference = normalizeReference(raw, current)
      if (reference === null) continue
      if (!descriptors.has(reference)) {
        throw new Error(`initial artifact dependency is absent from manifest: ${reference} (referenced by ${current})`)
      }
      if (!selected.has(reference)) queue.push(reference)
    }
  }
  const files = [...selected].sort().map((filePath) => {
    const bytes = bytesByPath.get(filePath)
    const compressed = brotliCompressSync(bytes, {
      params: { [zlibConstants.BROTLI_PARAM_QUALITY]: 11 },
    })
    return {
      path: filePath,
      bytes: bytes.byteLength,
      brotli_bytes: compressed.byteLength,
      sha256: descriptors.get(filePath).sha256,
    }
  })
  const brotliBytes = files.reduce((total, file) => total + file.brotli_bytes, 0)
  const payloadBytes = files.reduce((total, file) => total + file.bytes, 0)
  process.stdout.write(`${JSON.stringify({
    build_id: manifest.buildId,
    files,
    payload_bytes: payloadBytes,
    brotli_bytes: brotliBytes,
  })}\n`)
}

const args = process.argv.slice(2)
if (args.length === 1 && args[0] === "self-test") selfTest()
else if (args[0] === "write-json") await writeStdin(args)
else await artifact(parseRoot(args))
