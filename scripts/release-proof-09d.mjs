#!/usr/bin/env node

import { brotliCompressSync, constants as zlibConstants } from "node:zlib"
import assert from "node:assert/strict"
import { constants as fsConstants } from "node:fs"
import { lstat, open, readFile, rename, rm } from "node:fs/promises"
import path from "node:path"

function usage() {
  throw new Error("usage: release-proof-09d.mjs artifact --root <workspace> | write-json --path <file> | task-total --board-id <id> | query-evidence --stream-min <count> --reconnect-required <0|1> | self-test")
}

function nonArchivedTaskTotal(envelope, boardId) {
  assert(typeof boardId === "string" && boardId.length > 0, "canonical board ID is required")
  assert.equal(envelope?.data?.board_id, boardId, "stats board identity differs from the fixture board")
  assert(Array.isArray(envelope.data.status_counts), "stats must contain every populated status group")
  const statuses = new Set(["triage", "todo", "scheduled", "ready", "running", "blocked", "review", "done", "archived"])
  const seen = new Set()
  let total = 0
  for (const row of envelope.data.status_counts) {
    assert(row && statuses.has(row.status), "stats contains an unknown task status")
    assert(!seen.has(row.status), "stats contains a duplicate status group")
    assert(Number.isSafeInteger(row.count) && row.count >= 0, "stats count must be a non-negative safe integer")
    seen.add(row.status)
    // 与默认 ListTasks 的 status != 'archived' 过滤保持同一个任务集合。
    if (row.status !== "archived") total += row.count
    assert(Number.isSafeInteger(total), "stats total exceeds the exact integer range")
  }
  return total
}

function isQueryCursor(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value)
    && Object.keys(value).sort().join(",") === "epoch,revision,scope"
    && typeof value.epoch === "string" && value.epoch.length > 0 && value.epoch.length <= 128
    && typeof value.scope === "string" && value.scope.length > 0 && value.scope.length <= 256
    && typeof value.revision === "string" && /^[1-9][0-9]{0,19}$/.test(value.revision)
    && BigInt(value.revision) <= 18446744073709551615n
}

function sameQueryCursor(left, right) {
  return isQueryCursor(left) && isQueryCursor(right)
    && left.epoch === right.epoch && left.scope === right.scope && left.revision === right.revision
}

function validateQueryEvidence(evidence, streamMin, reconnectRequired) {
  const requests = evidence.query_stream_requests
  const reconnect = evidence.query_reconnect
  const finiteTime = value => typeof value === "number" && Number.isFinite(value) && value >= 0
  assert(Array.isArray(requests) && requests.length >= streamMin, "query stream requests are missing")
  assert(reconnect && typeof reconnect === "object", "query reconnect evidence is missing")
  assert(!("last_event_id" in reconnect) && !("after" in reconnect), "event IDs are not projection cursors")
  const origin = new URL(evidence.base_url).origin
  for (const request of requests) {
    const url = new URL(request.url)
    assert(url.origin === origin && url.pathname === "/kanban.v1.QueryService/WatchQueries"
      && url.search === "" && url.hash === "", "query request does not target the canonical QueryService")
    assert(typeof request.client_query_id === "string" && request.client_query_id.length > 0
      && request.client_query_id.length <= 128, "query request ID is missing")
    assert(finiteTime(request.at_ms), "query request timestamp is invalid")
    assert(request.resume === null || isQueryCursor(request.resume), "query request resume is not a complete cursor")
    assert(!("last_event_id" in request) && !("after" in request), "query requests cannot use event replay cursors")
  }
  assert.equal(evidence.query_contract?.budgets_ms?.mutation_to_event_p95, 1000, "query latency budget changed")
  assert.equal(evidence.query_contract?.budgets_ms?.disconnected_catch_up_p95, 3000, "query catch-up budget changed")
  if (evidence.query_contract.latency_budget_status === "passed") {
    const normal = evidence.query?.mutation_to_event_ms?.p95
    const catchUp = evidence.query?.disconnected_catch_up_ms?.p95
    assert(finiteTime(normal) && normal <= 1000, "query latency exceeds its declared passed budget")
    assert(finiteTime(catchUp) && catchUp <= 3000, "query catch-up exceeds its declared passed budget")
  }
  assert.equal(reconnect.new_request_after_disconnect, reconnectRequired, "query reconnect gate differs from the lane")
  assert.equal(reconnect.stale_notice_cleared, reconnectRequired, "query stale notice gate differs from the lane")
  if (!reconnectRequired) {
    assert.equal(reconnect.before_count, 0)
    assert.equal(reconnect.request_count_after_disconnect, 0)
    for (const key of ["request_at_ms", "event_seen_at_ms", "resume", "confirmed_cursor", "recovery_cursor", "recovery_mode", "confirmed_task_id", "confirmed_event_id"]) {
      assert.equal(reconnect[key], null, `inactive query reconnect field must be null: ${key}`)
    }
    return
  }
  assert(Number.isSafeInteger(reconnect.before_count) && reconnect.before_count >= 1, "confirmed query request count is invalid")
  assert(Number.isSafeInteger(reconnect.request_count_after_disconnect)
    && reconnect.request_count_after_disconnect > reconnect.before_count
    && reconnect.request_count_after_disconnect <= requests.length, "new query request after disconnect is unproven")
  assert(finiteTime(reconnect.request_at_ms) && finiteTime(reconnect.event_seen_at_ms)
    && reconnect.request_at_ms <= reconnect.event_seen_at_ms, "query recovery must precede visible catch-up")
  assert(isQueryCursor(reconnect.confirmed_cursor) && isQueryCursor(reconnect.recovery_cursor), "committed projection cursors are incomplete")
  for (const key of ["confirmed_task_id", "confirmed_event_id"]) {
    assert(typeof reconnect[key] === "string" && reconnect[key].length > 0, `confirmed business identity is missing: ${key}`)
  }
  if (reconnect.recovery_mode === "resume") {
    // 此模式描述请求携带已提交 cursor；Host 更换 epoch/scope 后仍可用完整 snapshot 重置。
    assert(sameQueryCursor(reconnect.resume, reconnect.confirmed_cursor), "resume must carry the last complete projection cursor")
  } else {
    assert.equal(reconnect.recovery_mode, "snapshot", "query recovery mode is missing")
    assert.equal(reconnect.resume, null, "fresh snapshot requests cannot claim an old resume cursor")
  }
  assert(requests.slice(reconnect.before_count).some(request => request.at_ms === reconnect.request_at_ms
    && (request.resume === null ? reconnect.resume === null : sameQueryCursor(request.resume, reconnect.resume))),
    "query reconnect evidence does not match an observed request")
}

function queryEvidenceSelfTest() {
  const cursor = { epoch: "host-epoch", scope: "query-scope", revision: "9" }
  const recovered = { ...cursor, revision: "10" }
  const request = { url: "http://127.0.0.1:18729/kanban.v1.QueryService/WatchQueries", at_ms: 1, client_query_id: "events", resume: null }
  const evidence = {
    base_url: "http://127.0.0.1:18729",
    query: { mutation_to_event_ms: { p95: 900 }, disconnected_catch_up_ms: { p95: 2900 } },
    query_contract: { latency_budget_status: "passed", budgets_ms: { mutation_to_event_p95: 1000, disconnected_catch_up_p95: 3000 } },
    query_stream_requests: [request, { ...request, at_ms: 2, resume: cursor }],
    query_reconnect: {
      new_request_after_disconnect: true, stale_notice_cleared: true, before_count: 1,
      request_count_after_disconnect: 2, request_at_ms: 2, event_seen_at_ms: 3,
      resume: cursor, confirmed_cursor: cursor, recovery_cursor: recovered, recovery_mode: "resume",
      confirmed_task_id: "t_confirmed", confirmed_event_id: "ev_confirmed",
    },
  }
  validateQueryEvidence(evidence, 2, true)
  const snapshot = structuredClone(evidence)
  snapshot.query_stream_requests[1].resume = null
  snapshot.query_reconnect.resume = null
  snapshot.query_reconnect.recovery_mode = "snapshot"
  snapshot.query_reconnect.recovery_cursor = { epoch: "new-host", scope: "new-owner", revision: "1" }
  validateQueryEvidence(snapshot, 2, true)
  const inactive = structuredClone(evidence)
  inactive.query = null
  inactive.query_contract.latency_budget_status = "not_run"
  inactive.query_stream_requests = []
  inactive.query_reconnect = {
    new_request_after_disconnect: false, stale_notice_cleared: false, before_count: 0,
    request_count_after_disconnect: 0, request_at_ms: null, event_seen_at_ms: null,
    resume: null, confirmed_cursor: null, recovery_cursor: null, recovery_mode: null,
    confirmed_task_id: null, confirmed_event_id: null,
  }
  validateQueryEvidence(inactive, 0, false)
  for (const change of [
    value => { delete value.query_stream_requests },
    value => { value.query_stream_requests[1].url = "http://127.0.0.1:18729/api/v1/events/stream" },
    value => { value.query_reconnect.resume = "ev_confirmed" },
    value => { value.query_reconnect.confirmed_cursor = { revision: "9" } },
    value => { value.query_reconnect.recovery_cursor.revision = 10 },
    value => { value.query_reconnect.recovery_cursor.revision = "010" },
    value => { value.query_reconnect.recovery_cursor.revision = "18446744073709551616" },
    value => { value.query_reconnect.resume = { ...cursor, revision: "8" } },
    value => { value.query_reconnect.last_event_id = "ev_confirmed" },
    value => { value.query_reconnect.recovery_mode = "snapshot" },
    value => { value.query_reconnect.request_at_ms = 4 },
    value => { value.query_contract.budgets_ms.mutation_to_event_p95 = 2000 },
    value => { value.query.mutation_to_event_ms.p95 = 1001 },
    value => { value.query.disconnected_catch_up_ms.p95 = 3001 },
  ]) {
    const invalid = structuredClone(evidence)
    change(invalid)
    assert.throws(() => validateQueryEvidence(invalid, 2, true), "invalid query evidence must fail closed")
  }
  const statuses = ["triage", "todo", "scheduled", "ready", "running", "blocked", "review", "done", "archived"]
  const stats = { data: { board_id: "b_fixture", status_counts: statuses.map((status, index) => ({ status, count: index + 1 })) } }
  assert.equal(nonArchivedTaskTotal(stats, "b_fixture"), 36)
  assert.equal(nonArchivedTaskTotal({ data: { board_id: "b_fixture", status_counts: [] } }, "b_fixture"), 0)
  assert.throws(() => nonArchivedTaskTotal(stats, "b_other"))
  for (const row of [{ status: "todo", count: 1 }, { status: "unknown", count: 1 }, { status: "new", count: -1 }]) {
    assert.throws(() => nonArchivedTaskTotal({ data: { ...stats.data, status_counts: [...stats.data.status_counts, row] } }, "b_fixture"))
  }
  assert.throws(() => nonArchivedTaskTotal({ data: { board_id: "b_fixture", status_counts: [{ status: "todo", count: -1 }] } }, "b_fixture"))
  assert.throws(() => nonArchivedTaskTotal({ data: { board_id: "b_fixture", status_counts: [{ status: "todo", count: 1.5 }] } }, "b_fixture"))
  assert.throws(() => nonArchivedTaskTotal({ data: { board_id: "b_fixture", status_counts: [{ status: "todo", count: "1" }] } }, "b_fixture"))
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
  queryEvidenceSelfTest()
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
else if (args[0] === "task-total" || args[0] === "query-evidence") {
  const chunks = []
  for await (const chunk of process.stdin) chunks.push(Buffer.from(chunk))
  const value = JSON.parse(Buffer.concat(chunks).toString("utf8"))
  if (args[0] === "task-total") {
    if (args.length !== 3 || args[1] !== "--board-id") usage()
    process.stdout.write(`${nonArchivedTaskTotal(value, args[2])}\n`)
  } else {
    if (args.length !== 5 || args[1] !== "--stream-min" || !/^[0-9]+$/.test(args[2])
      || args[3] !== "--reconnect-required" || !["0", "1"].includes(args[4])) usage()
    const streamMin = Number(args[2])
    assert(Number.isSafeInteger(streamMin), "query stream minimum exceeds the exact integer range")
    validateQueryEvidence(value, streamMin, args[4] === "1")
  }
}
else await artifact(parseRoot(args))
