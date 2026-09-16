#!/usr/bin/env node

import { brotliCompressSync, constants as zlibConstants } from "node:zlib"
import assert from "node:assert/strict"
import { constants as fsConstants } from "node:fs"
import { lstat, open, readFile, rename, rm } from "node:fs/promises"
import path from "node:path"

function usage() {
  throw new Error("usage: release-proof-09d.mjs artifact --root <workspace> | write-json --path <file> | task-total --board-id <id> | query-evidence --stream-min <count> --reconnect-required <0|1> | ui-evidence --kind <none|functional_2k|stress_5k|all> | self-test")
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

const listStatuses = ["running", "blocked", "review", "ready", "scheduled", "todo", "triage", "done", "archived"]
const atlasColumns = [
  ["待开始", ["triage", "todo", "scheduled", "ready"]],
  ["进行中", ["running", "blocked"]], ["待验收", ["review"]], ["已完成", ["done", "archived"]],
]
const defaultQuery = { page: 1, limit: 100, sort: "updated_at", status: [], q: "" }

function checkedIds(ids) {
  assert(Array.isArray(ids) && ids.every(id => typeof id === "string" && id.length > 0), "task identities are missing")
  assert.equal(new Set(ids).size, ids.length, "a task window contains duplicate identities")
  return ids
}

function sameIds(actual, expected) {
  assert.deepEqual([...checkedIds(actual)].sort(), [...checkedIds(expected)].sort(), "UI and canonical task identities differ")
}

function validateWindow(window, origin, view, query, total) {
  assert(window && typeof window === "object", "task window evidence is missing")
  assert.equal(window.view, view)
  assert.deepEqual(window.query, query)
  const url = new URL(window.url)
  assert.equal(url.origin, origin, "task window belongs to another Host")
  assert.equal(url.pathname, `/app/boards/default/${view}`)
  assert.equal(url.hash, "")
  assert([...url.searchParams.keys()].every(key => ["page", "limit", "sort", "status", "q"].includes(key)), "unproven query filters")
  assert.deepEqual({
    page: Number(url.searchParams.get("page") ?? 1), limit: Number(url.searchParams.get("limit") ?? 100),
    sort: url.searchParams.get("sort") ?? "updated_at", status: url.searchParams.getAll("status"), q: url.searchParams.get("q") ?? "",
  }, query, "window query differs from the observed URL")
  const canonical = window.canonical
  assert.equal(canonical?.total, total)
  assert.equal(canonical.offset, (query.page - 1) * query.limit)
  assert.equal(canonical.limit, query.limit)
  assert(query.limit > 0 && query.limit <= 100, "Atlas window exceeds the 100-task cap")
  assert(Array.isArray(canonical.tasks), "canonical ListTasks result is missing")
  assert(canonical.tasks.every(task => listStatuses.includes(task.status) && typeof task.title === "string"), "canonical task fields are incomplete")
  const canonicalIds = checkedIds(canonical.tasks.map(task => task.id))
  const count = Math.min(query.limit, total - canonical.offset)
  assert(count > 0 && canonicalIds.length === count, "canonical window is incomplete")
  sameIds(window.task_ids, canonicalIds)
  assert.equal(window.displayed_count, count)
  assert.equal(window.displayed_total, total)
  const footer = window.footer_text?.match(/^显示 (\d+) \/ (\d+) 个任务/)
  assert(footer && Number(footer[1]) === count && Number(footer[2]) === total, "visible footer count/total differs from canonical")
  const totalPages = Math.ceil(total / query.limit)
  assert.equal(window.pager_text, `${query.page} / ${totalPages}`)
  assert.equal(window.page, query.page)
  assert.equal(window.total_pages, totalPages)
  assert.equal(window.previous_enabled, query.page > 1)
  assert.equal(window.next_enabled, query.page < totalPages)
  if (view === "list") {
    assert.equal(window.columns, null)
    const orderedIds = listStatuses.flatMap(status => canonical.tasks.filter(task => task.status === status).map(task => task.id))
    assert.deepEqual(window.task_ids, orderedIds, "list grouping changed the canonical order within a status")
  } else {
    assert.equal(window.columns?.length, atlasColumns.length)
    assert.deepEqual(window.columns.flatMap(column => column.task_ids), window.task_ids)
    atlasColumns.forEach(([label, statuses], index) => {
      const column = window.columns[index]
      const expected = canonical.tasks.filter(task => statuses.includes(task.status))
      assert.equal(column.label, label)
      assert.equal(column.count_text, String(expected.length))
      assert.equal(column.displayed_count, expected.length)
      sameIds(column.task_ids, expected.map(task => task.id))
      assert.deepEqual(column.task_statuses, column.task_ids.map(id => expected.find(task => task.id === id).status))
    })
  }
}

function validateUiEvidence(evidence, kind) {
  if (kind === "none") { assert.equal(evidence.ui, null); return }
  assert(["functional_2k", "stress_5k"].includes(kind), "unknown UI evidence lane")
  const total = kind === "functional_2k" ? 2000 : 5000
  assert.equal(evidence.phase, kind.replace("_", "-"))
  const origin = new URL(evidence.base_url).origin
  const ui = evidence.ui?.[kind]
  assert(ui, "Atlas UI evidence is missing")
  assert.equal(evidence.ui[kind === "functional_2k" ? "stress_5k" : "functional_2k"], null)
  assert.equal(Number(ui.stats?.text?.match(/^(\d+)\s+个任务$/)?.[1]), total, "visible Stats total differs from canonical")
  assert.equal(ui.stats.total, total)
  assert.equal(ui.stats.canonical_total, total)
  assert.equal(ui.board_ready?.budget_ms, 120000, "board ready budget changed")
  assert.equal(ui.board_ready.within_budget, true)
  assert(Number.isFinite(ui.board_ready.navigation_start_ms) && ui.board_ready.navigation_start_ms >= 0)
  assert(Number.isFinite(ui.board_ready.ready_ms) && ui.board_ready.ready_ms >= 0 && ui.board_ready.ready_ms <= 120000)
  assert.equal(ui.list_pages?.length, total / 100, "full UI pagination coverage is missing")
  ui.list_pages.forEach((window, index) => validateWindow(window, origin, "list", { ...defaultQuery, page: index + 1 }, total))
  const allIds = checkedIds(ui.list_pages.flatMap(window => window.task_ids))
  assert.equal(allIds.length, total, "UI pages omit tasks")
  assert.equal(ui.board_pages?.length, 2)
  ui.board_pages.forEach((window, index) => {
    validateWindow(window, origin, "board", { ...defaultQuery, page: index + 1 }, total)
    sameIds(window.task_ids, ui.list_pages[index].task_ids)
    assert.deepEqual(window.canonical, ui.list_pages[index].canonical)
  })
  checkedIds(ui.board_pages.flatMap(window => window.task_ids))
  const filtered = ui.filter_sort
  assert.equal(filtered?.status_text, "待开始")
  assert.equal(filtered.search_text, "load task 1")
  assert.equal(filtered.sort_text, "标题")
  assert.equal(filtered.changed_sort_text, "最近更新")
  assert.equal(filtered.pages?.length, 2)
  const filteredQuery = { ...defaultQuery, status: ["todo"], q: "load task 1", sort: "title" }
  filtered.pages.forEach((window, index) => validateWindow(window, origin, "list", { ...filteredQuery, page: index + 1 }, 1000))
  checkedIds(filtered.pages.flatMap(window => window.task_ids))
  const filteredTasks = filtered.pages.flatMap(window => window.canonical.tasks)
  assert(filteredTasks.every(task => allIds.includes(task.id)), "filter window contains tasks absent from the full listing")
  assert(filteredTasks.every(task => task.status === "todo" && task.title.includes("load task 1")), "filter results contain unrelated tasks")
  const titles = filteredTasks.map(task => task.title)
  assert.deepEqual(titles, [...titles].sort(), "title sort is not ascending across pages")
  validateWindow(filtered.changed_sort, origin, "list", { ...filteredQuery, sort: "updated_at" }, 1000)
  assert(filtered.changed_sort.task_ids.every(id => allIds.includes(id)))
  assert(filtered.changed_sort.canonical.tasks.every(task => task.status === "todo" && task.title.includes("load task 1")))
  assert.notDeepEqual(filtered.changed_sort.task_ids, filtered.pages[0].task_ids, "sort change did not change the visible window")
  validateWindow(filtered.cleared, origin, "list", defaultQuery, total)
  assert.deepEqual(filtered.cleared.task_ids, ui.list_pages[0].task_ids, "clearing filters did not restore the full first page")
  if (kind === "stress_5k") {
    const size = ui.page_size
    assert.deepEqual(size?.options, ["每页 10 项", "每页 25 项", "每页 50 项", "每页 100 项"])
    assert.equal(size.before_text, "每页 50 项")
    assert.equal(size.after_text, "每页 100 项")
    assert.equal(size.before_pages?.length, 2)
    size.before_pages.forEach((window, index) => validateWindow(window, origin, "list", { ...defaultQuery, limit: 50, page: index + 1 }, total))
    validateWindow(size.after, origin, "list", defaultQuery, total)
    sameIds(size.before_pages.flatMap(window => window.task_ids), size.after.task_ids)
    assert.deepEqual(size.after.task_ids, ui.list_pages[0].task_ids)
    const map = evidence.map
    assert(map && Number.isSafeInteger(map.node_count) && map.node_count > 0 && map.node_count <= 240)
    assert(Number.isSafeInteger(map.edge_count) && map.edge_count >= 0)
    assert.equal(map.limit_nodes, 240)
    assert.equal(typeof map.truncated, "boolean")
    assert.equal(map.ui_node_count, map.node_count)
    assert.equal(map.ui_edge_count, map.edge_count)
    assert.equal(map.ui_truncated, map.truncated)
    assert.equal(map.zoom_before, 100)
    assert.equal(map.zoom_after, 115)
    for (const key of ["node_count", "edge_count", "truncated", "limit_nodes", "zoom_before", "zoom_after"]) assert.equal(ui.map?.[key], map[key])
  }
}

function uiEvidenceSelfTest() {
  const total = 2000, origin = "http://127.0.0.1:18729"
  const window = (view, query, countTotal = total, reverse = false) => {
    const offset = (query.page - 1) * query.limit
    const tasks = Array.from({ length: query.limit }, (_, index) => {
      const sequence = String(reverse ? 1999 - index : query.q ? 1000 + offset + index : offset + index).padStart(4, "0")
      return { id: `t_${sequence}`, status: "todo", title: `Stage09 09D load task ${sequence}` }
    })
    const ids = tasks.map(task => task.id)
    const params = new URLSearchParams({ page: String(query.page), limit: String(query.limit), sort: query.sort })
    query.status.forEach(status => params.append("status", status))
    if (query.q) params.set("q", query.q)
    const totalPages = Math.ceil(countTotal / query.limit)
    return {
      view, url: `${origin}/app/boards/default/${view}?${params}`, query,
      footer_text: `显示 ${ids.length} / ${countTotal} 个任务`, displayed_count: ids.length, displayed_total: countTotal,
      pager_text: `${query.page} / ${totalPages}`, page: query.page, total_pages: totalPages,
      previous_enabled: query.page > 1, next_enabled: query.page < totalPages,
      task_ids: ids, canonical: { total: countTotal, offset, limit: query.limit, tasks },
      columns: view === "list" ? null : atlasColumns.map(([label], index) => ({ label, count_text: String(index === 0 ? ids.length : 0), displayed_count: index === 0 ? ids.length : 0, task_ids: index === 0 ? ids : [], task_statuses: index === 0 ? ids.map(() => "todo") : [] })),
    }
  }
  const filtered = { ...defaultQuery, status: ["todo"], q: "load task 1", sort: "title" }
  const makeUi = () => ({
    stats: { text: `${total}\n个任务`, total, canonical_total: total },
    board_ready: { navigation_start_ms: 0, ready_ms: 1000, budget_ms: 120000, within_budget: true },
    board_pages: [1, 2].map(page => window("board", { ...defaultQuery, page })),
    list_pages: Array.from({ length: 20 }, (_, index) => window("list", { ...defaultQuery, page: index + 1 })),
    filter_sort: { status_text: "待开始", search_text: "load task 1", sort_text: "标题", changed_sort_text: "最近更新",
      pages: [1, 2].map(page => window("list", { ...filtered, page }, 1000)),
      changed_sort: window("list", { ...filtered, sort: "updated_at" }, 1000, true), cleared: window("list", defaultQuery) },
  })
  const evidence = { phase: "functional-2k", base_url: origin, ui: { functional_2k: makeUi(), stress_5k: null } }
  validateUiEvidence(evidence, "functional_2k")
  for (const change of [
    value => { value.list_pages.pop() },
    value => { value.list_pages[1].task_ids[0] = value.list_pages[0].task_ids[0] },
    value => { value.list_pages[1].task_ids = value.list_pages[0].task_ids; value.list_pages[1].canonical.tasks = value.list_pages[0].canonical.tasks },
    value => { value.list_pages[0].footer_text = "显示 100 / 1999 个任务" },
    value => { value.list_pages[0].canonical.total = 1999 },
    value => { value.list_pages[0].query.limit = 200 },
    value => { value.list_pages[0].url = value.list_pages[0].url.replace("18729", "18730") },
    value => { value.list_pages[1].previous_enabled = false },
    value => { value.board_pages[0].columns[0].count_text = "2000" },
    value => { value.filter_sort.pages[1].task_ids.reverse() },
    value => { value.filter_sort.changed_sort = value.filter_sort.pages[0] },
    value => { value.board_ready.budget_ms = 240000 },
  ]) {
    const invalid = structuredClone(evidence)
    change(invalid.ui.functional_2k)
    assert.throws(() => validateUiEvidence(invalid, "functional_2k"), "invalid Atlas UI evidence was accepted")
  }
  const stressUi = makeUi()
  stressUi.stats = { text: "5000 个任务", total: 5000, canonical_total: 5000 }
  stressUi.list_pages = Array.from({ length: 50 }, (_, index) => window("list", { ...defaultQuery, page: index + 1 }, 5000))
  stressUi.board_pages = [1, 2].map(page => window("board", { ...defaultQuery, page }, 5000))
  stressUi.filter_sort.cleared = window("list", defaultQuery, 5000)
  stressUi.page_size = {
    options: ["每页 10 项", "每页 25 项", "每页 50 项", "每页 100 项"], before_text: "每页 50 项", after_text: "每页 100 项",
    before_pages: [1, 2].map(page => window("list", { ...defaultQuery, limit: 50, page }, 5000)), after: window("list", defaultQuery, 5000),
  }
  stressUi.map = { node_count: 240, edge_count: 0, truncated: true, limit_nodes: 240, zoom_before: 100, zoom_after: 115 }
  const stress = {
    phase: "stress-5k", base_url: origin, ui: { functional_2k: null, stress_5k: stressUi },
    map: { ...stressUi.map, ui_node_count: 240, ui_edge_count: 0, ui_truncated: true },
  }
  validateUiEvidence(stress, "stress_5k")
  for (const change of [
    value => { value.ui.stress_5k.page_size.options.push("每页 200 项") },
    value => { value.ui.stress_5k.page_size.before_pages.pop() },
    value => { value.ui.stress_5k.page_size.after = value.ui.stress_5k.list_pages[1] },
    value => { value.map.node_count = 241 },
    value => { value.map.ui_node_count = 239 },
    value => { value.map.ui_truncated = false },
    value => { value.map.zoom_after = 100 },
    value => { value.ui.stress_5k.map.limit_nodes = 500 },
  ]) {
    const invalid = structuredClone(stress)
    change(invalid)
    assert.throws(() => validateUiEvidence(invalid, "stress_5k"), "invalid stress UI evidence was accepted")
  }
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
  uiEvidenceSelfTest()
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
else if (["task-total", "query-evidence", "ui-evidence"].includes(args[0])) {
  const chunks = []
  for await (const chunk of process.stdin) chunks.push(Buffer.from(chunk))
  const value = JSON.parse(Buffer.concat(chunks).toString("utf8"))
  if (args[0] === "task-total") {
    if (args.length !== 3 || args[1] !== "--board-id") usage()
    process.stdout.write(`${nonArchivedTaskTotal(value, args[2])}\n`)
  } else if (args[0] === "ui-evidence") {
    if (args.length !== 3 || args[1] !== "--kind") usage()
    if (args[2] === "all") {
      assert(Array.isArray(value.browsers) && value.browsers.length === 6)
      for (const browser of value.browsers) {
        const kind = browser.phase === "functional-2k" ? "functional_2k" : browser.phase === "stress-5k" ? "stress_5k" : "none"
        validateUiEvidence(browser, kind)
      }
    } else validateUiEvidence(value, args[2])
  } else {
    if (args.length !== 5 || args[1] !== "--stream-min" || !/^[0-9]+$/.test(args[2])
      || args[3] !== "--reconnect-required" || !["0", "1"].includes(args[4])) usage()
    const streamMin = Number(args[2])
    assert(Number.isSafeInteger(streamMin), "query stream minimum exceeds the exact integer range")
    validateQueryEvidence(value, streamMin, args[4] === "1")
  }
}
else await artifact(parseRoot(args))
