import { readFileSync } from "node:fs"
import { afterEach, describe, expect, it, vi } from "vitest"

import { setCurrentDesktopLocale } from "@/i18n"
import { KanbanApi, loadRuntimeConfig, type ApiError, type Board, type SearchIndexStatus, type SearchTasksMeta, type Task } from "./api"

const runtimeConfig = {
  apiBaseUrl: "http://127.0.0.1:8721",
  dbPath: "test.db",
  actor: "desktop-test",
  board: "default",
}

const apiSource = readFileSync(new URL("./api.ts", import.meta.url), "utf8")

describe("KanbanApi task search", () => {
  afterEach(() => {
    vi.unstubAllGlobals()
    setCurrentDesktopLocale("en")
  })

  it("consumes committed C2b task-read fixtures through production clients", async () => {
    const listFixture = JSON.parse(readFileSync(new URL("../../../../schemas/fixtures/api/list-tasks-response.v1.valid.json", import.meta.url), "utf8"))
    const windowsFixture = JSON.parse(readFileSync(new URL("../../../../schemas/fixtures/api/list-tasks-by-status-response.v1.valid.json", import.meta.url), "utf8"))
    const api = new KanbanApi(runtimeConfig)
    const listFetch = mockFetch(listFixture)
    const list = await api.listTasks({ limit: 25, offset: 0 })
    expect(list.page).toEqual({ limit: 25, offset: 0, total: 1 })
    expect(list.tasks).toHaveLength(1)
    vi.unstubAllGlobals()
    const windowsFetch = mockFetch(windowsFixture)
    const windows = await api.listTasksByStatus({ statuses: ["ready", "blocked"], limit: 25, offset: 0 })
    expect(windows.statuses.map((entry) => entry.page)).toEqual([{ limit: 25, offset: 0, total: 1 }, { limit: 25, offset: 0, total: 0 }])
    expect(calledUrl(listFetch).pathname).toContain("/tasks")
    expect(calledUrl(windowsFetch).pathname).toContain("/tasks/by-status")
  })

  it("keeps the task list endpoint query-free for empty search flows", async () => {
    const fetchMock = mockFetch({ data: [task({ id: "t_list", title: "plain list" })], meta: { limit: 25, offset: 50, total: 1 } })
    const api = new KanbanApi(runtimeConfig)

    const result = await api.listTasks({ includeArchived: true, statuses: ["ready"], limit: 25, offset: 50 })

    expect(result.tasks).toHaveLength(1)
    expect(result.page).toEqual({ limit: 25, offset: 50, total: 1 })
    const url = calledUrl(fetchMock)
    expect(url.pathname).toBe("/api/v1/boards/default/tasks")
    expect(url.searchParams.get("q")).toBeNull()
    expect(url.searchParams.get("search")).toBeNull()
    expect(url.searchParams.get("include_archived")).toBe("true")
    expect(url.searchParams.get("limit")).toBe("25")
    expect(url.searchParams.get("offset")).toBe("50")
    expect(url.searchParams.getAll("status")).toEqual(["ready"])
  })

  it("passes list search, priority filters, and sort to the task list endpoint", async () => {
    const fetchMock = mockFetch({ data: [], meta: { limit: 25, offset: 0, total: 0 } })
    const api = new KanbanApi(runtimeConfig)

    await api.listTasks({
      query: " 架构 & /=+ 空格 ",
      statuses: ["ready", "blocked"],
      priorities: [0, 2],
      labels: [" 后端 & API ", "api=客户端+v1"],
      planFilters: ["plan_needed", "incomplete_required_steps"],
      sort: "priority",
      limit: 25,
      offset: 0,
    })

    const url = calledUrl(fetchMock)
    expect(url.pathname).toBe("/api/v1/boards/default/tasks")
    expect(url.searchParams.get("q")).toBe("架构 & /=+ 空格")
    expect(url.searchParams.get("search")).toBeNull()
    expect(url.searchParams.get("sort")).toBe("priority")
    expect(url.searchParams.getAll("status")).toEqual(["ready", "blocked"])
    expect(url.searchParams.getAll("priority")).toEqual(["0", "2"])
    expect(url.searchParams.getAll("label")).toEqual(["后端 & API", "api=客户端+v1"])
    expect(url.searchParams.getAll("plan_filter")).toEqual(["plan_needed", "incomplete_required_steps"])
  })

  it("keeps list task pagination metadata from the response envelope", async () => {
    const fetchMock = mockFetch({
      data: [task({ id: "t_list", title: "plain list" })],
      meta: { limit: 25, offset: 50, total: 225 },
    })
    const api = new KanbanApi(runtimeConfig)

    const result = await api.listTasks({ includeArchived: true, limit: 25, offset: 50 })

    expect(result.page).toEqual({ limit: 25, offset: 50, total: 225 })
    expect(calledInit(fetchMock).signal).toBeUndefined()
  })

  it("uses the search endpoint and returns hydrated task rows", async () => {
    const hitTask = task({ id: "t_search", title: "hydrated search result" })
    const searchMeta = {
      backend: "tantivy",
      stale: true,
      index_version: "v1",
      last_event_id: 12,
      index_lag_events: 2,
    } satisfies SearchTasksMeta
    const fetchMock = mockFetch({
      data: {
        hits: [{ task_id: hitTask.id, seq: hitTask.seq, score: 4.2, snippet: "hydrated", task: hitTask }],
        meta: searchMeta,
      },
    })
    const api = new KanbanApi(runtimeConfig)

    const result = await api.searchTasks({
      query: " hydrated ",
      includeArchived: false,
      statuses: ["ready", "review"],
      labels: [" backend ", "api"],
      limit: 20,
      offset: 40,
    })

    expect(result.tasks).toEqual([hitTask])
    expect(result.searchMeta).toEqual(searchMeta)
    expect("derived_index" in result.searchMeta).toBe(false)
    expect("message" in result.searchMeta).toBe(false)
    expect(result.page).toEqual({ limit: 20, offset: 40, total: null })
    const url = calledUrl(fetchMock)
    expect(url.pathname).toBe("/api/v1/search/tasks")
    expect(url.searchParams.get("board")).toBe("default")
    expect(url.searchParams.get("q")).toBe("hydrated")
    expect(url.searchParams.get("limit")).toBe("20")
    expect(url.searchParams.get("offset")).toBe("40")
    expect(url.searchParams.getAll("status")).toEqual(["ready", "review"])
    expect(url.searchParams.getAll("label")).toEqual(["backend", "api"])
  })

  it("uses batch task windows by status", async () => {
    setCurrentDesktopLocale("zh-CN")
    const ready = task({ id: "t_ready", status: "ready" })
    const blocked = task({ id: "t_blocked", status: "blocked" })
    const fetchMock = mockFetch({
      data: {
        statuses: [
          { status: "ready", tasks: [ready], page: { limit: 50, offset: 0, total: 1 } },
          { status: "blocked", tasks: [blocked], page: { limit: 50, offset: 0, total: 1 } },
        ],
      },
      meta: { limit: 50, offset: 0 },
    })
    const api = new KanbanApi(runtimeConfig)

    const result = await api.listTasksByStatus({ statuses: ["ready", "blocked"], includeArchived: false, limit: 50 })

    expect(result.statuses.map((entry) => entry.status)).toEqual(["ready", "blocked"])
    expect(result.statuses.flatMap((entry) => entry.tasks)).toEqual([ready, blocked])
    const url = calledUrl(fetchMock)
    expect(url.pathname).toBe("/api/v1/boards/default/tasks/by-status")
    expect(url.searchParams.get("search")).toBeNull()
    expect(url.searchParams.getAll("status")).toEqual(["ready", "blocked"])
    expect(url.searchParams.get("limit")).toBe("50")
    expect(calledInit(fetchMock).headers).toEqual({ "Accept-Language": "zh-CN" })
  })

  it("sends the configured locale on API requests", async () => {
    const fetchMock = mockFetch({ data: [task({ id: "t_list", title: "localized list" })], meta: { limit: 10, offset: 0, total: 1 } })
    const api = new KanbanApi(runtimeConfig, { locale: "en" })

    await api.listTasks({ includeArchived: false, limit: 10 })

    expect(calledInit(fetchMock).headers).toEqual({ "Accept-Language": "en" })
  })

  it("keeps actor and content headers while sending locale on mutations", async () => {
    const fetchMock = mockFetch({ data: task({ id: "t_created", title: "Created" }) })
    const api = new KanbanApi(runtimeConfig, { locale: "zh-CN" })

    await api.createTask({ title: "Created" })

    expect(calledInit(fetchMock).headers).toEqual({
      "Accept-Language": "zh-CN",
      "Content-Type": "application/json",
      "X-KB-Actor": "desktop-test",
    })
  })

  it("uses batch search windows by status", async () => {
    const ready = task({ id: "t_search_ready", status: "ready" })
    const searchMeta = {
      backend: "sqlite",
      stale: false,
      index_version: null,
      last_event_id: null,
      index_lag_events: null,
    } satisfies SearchTasksMeta
    const fetchMock = mockFetch({
      data: {
        statuses: [
          { status: "ready", tasks: [ready], search_meta: searchMeta, page: { limit: 50, offset: 0, total: null } },
        ],
      },
      meta: { limit: 50, offset: 0 },
    })
    const api = new KanbanApi(runtimeConfig)

    const result = await api.searchTasksByStatus({ query: "needle", statuses: ["ready"], includeArchived: false, limit: 50 })

    expect(result.statuses[0]).toMatchObject({ status: "ready", tasks: [ready], searchMeta })
    const url = calledUrl(fetchMock)
    expect(url.pathname).toBe("/api/v1/search/tasks/by-status")
    expect(url.searchParams.get("q")).toBe("needle")
    expect(url.searchParams.getAll("status")).toEqual(["ready"])
  })

  it("passes AbortSignal through queryable API requests", async () => {
    const fetchMock = mockFetch({ data: [], meta: { limit: 10, offset: 0, total: 0 } })
    const api = new KanbanApi(runtimeConfig)
    const controller = new AbortController()

    await api.listTasks({ signal: controller.signal, limit: 10 })

    expect(calledInit(fetchMock).signal).toBe(controller.signal)
  })

  it("rejects malformed task list envelopes before React consumes them", async () => {
    mockFetch({ data: { not: "an array" }, meta: { limit: 10, offset: 0, total: 1 } })
    const api = new KanbanApi(runtimeConfig)

    await expect(api.listTasks({ limit: 10 })).rejects.toMatchObject({
      code: "invalid_response",
      message: "tasks response data must be an array",
    } satisfies Partial<ApiError>)
  })


  it.each([
    ["extra envelope field", { data: [task()], meta: { limit: 10, offset: 0, total: 1 }, extra: true }],
    ["unsafe pagination", { data: [task()], meta: { limit: Number.MAX_SAFE_INTEGER + 1, offset: 0, total: 1 } }],
    ["fractional pagination", { data: [task()], meta: { limit: 10.5, offset: 0, total: 1 } }],
    ["unknown task status", { data: [task({ status: "mystery" as Task["status"] })], meta: { limit: 10, offset: 0, total: 1 } }],
    ["nested claim token", { data: [{ ...task(), claim_token: "secret" }], meta: { limit: 10, offset: 0, total: 1 } }],
    ["missing required nullable", { data: [{ ...task(), description: undefined }], meta: { limit: 10, offset: 0, total: 1 } }],
    ["invalid label", { data: [task({ labels: [{ id: "l_1", board_id: "b_1", name: "x", color: null, created_at: 1, updated_at: 1, extra: true } as never] })], meta: { limit: 10, offset: 0, total: 1 } }],
  ])("fails closed for hostile list response: %s", async (_label, payload) => {
    mockFetch(payload)
    await expect(new KanbanApi(runtimeConfig).listTasks({ limit: 10 })).rejects.toMatchObject({ code: "invalid_response" })
  })

  it.each([
    ["extra window field", { data: { statuses: [{ status: "ready", tasks: [task()], page: { limit: 10, offset: 0, total: 1 }, extra: true }] }, meta: { limit: 10, offset: 0 } }],
    ["unknown window status", { data: { statuses: [{ status: "mystery", tasks: [], page: { limit: 10, offset: 0, total: 0 } }] }, meta: { limit: 10, offset: 0 } }],
    ["negative window total", { data: { statuses: [{ status: "ready", tasks: [], page: { limit: 10, offset: 0, total: -1 } }] }, meta: { limit: 10, offset: 0 } }],
    ["extra data field", { data: { statuses: [], extra: true }, meta: { limit: 10, offset: 0 } }],
  ])("fails closed for hostile status-window response: %s", async (_label, payload) => {
    mockFetch(payload)
    await expect(new KanbanApi(runtimeConfig).listTasksByStatus({ statuses: ["ready"], limit: 10 })).rejects.toMatchObject({ code: "invalid_response" })
  })


  it.each([
    ["null body", { error: null }],
    ["string body", { error: "bad" }],
    ["missing message", { error: { code: "invalid_input" } }],
    ["extra envelope field", { error: { code: "invalid_input", message: "bad" }, extra: true }],
    ["mixed success and error", { data: [], error: { code: "invalid_input", message: "bad" } }],
    ["extra error field", { error: { code: "invalid_input", message: "bad", unknown: true } }],
  ])("rejects malformed task-read error envelope: %s", async (_label, payload) => {
    mockFetch(payload)
    await expect(new KanbanApi(runtimeConfig).listTasks({ limit: 10 })).rejects.toMatchObject({ code: "invalid_response" })
  })

  it("preserves a closed task-read error code, message, and details", async () => {
    const details = { field: "limit", reason: "too_large" }
    mockFetch({ error: { code: "invalid_input", message: "bad limit", details } })
    await expect(new KanbanApi(runtimeConfig).listTasks({ limit: 10 })).rejects.toMatchObject({
      code: "invalid_input",
      message: "bad limit",
      details,
    })
  })

  it("rejects malformed search envelopes before returning hydrated rows", async () => {
    mockFetch({
      data: {
        meta: {
          backend: "tantivy",
          stale: false,
          index_version: "v1",
          last_event_id: 12,
          index_lag_events: 0,
        },
      },
    })
    const api = new KanbanApi(runtimeConfig)

    await expect(api.searchTasks({ query: "broken" })).rejects.toMatchObject({
      code: "invalid_response",
      message: "search hits must be an array",
    } satisfies Partial<ApiError>)
  })

  it("preserves unknown totals while keeping numeric limit and offset", async () => {
    const searchMeta = {
      backend: "sqlite",
      stale: false,
      index_version: null,
      last_event_id: null,
      index_lag_events: null,
    } satisfies SearchTasksMeta
    const fetchMock = mockFetch({
      data: {
        hits: [],
        meta: searchMeta,
      },
      meta: { limit: 10, offset: 20 },
    })
    const api = new KanbanApi(runtimeConfig)

    const result = await api.searchTasks({ query: "missing total", limit: 10, offset: 20 })

    expect(result.page).toEqual({ limit: 10, offset: 20, total: null })
    expect(result.searchMeta).toEqual(searchMeta)
    expect(calledUrl(fetchMock).searchParams.get("q")).toBe("missing total")
  })

  it("uses event envelope cursor metadata instead of deriving only from row ids", async () => {
    const fetchMock = mockFetch({
      data: [eventRecord({ id: 123, task_id: "t_1", kind: "task.claimed" })],
      meta: { next_after: 130 },
    })
    const api = new KanbanApi(runtimeConfig)

    const page = await api.listEventsAfter(120)

    expect(page.events).toHaveLength(1)
    expect(page.meta.next_after).toBe(130)
    const url = calledUrl(fetchMock)
    expect(url.searchParams.get("after")).toBe("120")
  })

  it("uses /health outside the API v1 prefix", async () => {
    const fetchMock = mockFetch({ data: { ok: true, db: "ok", version: "1.1.0" } })
    const api = new KanbanApi(runtimeConfig)

    const health = await api.health()

    expect(health).toEqual({ ok: true, db: "ok", version: "1.1.0" })
    expect(calledUrl(fetchMock).pathname).toBe("/health")
  })

  it("adds and removes task labels through task label routes", async () => {
    const updated = task({
      labels: [
        { id: "l_backend", board_id: "b_1", name: "backend", color: null, created_at: 1, updated_at: 1 },
      ],
    })
    const fetchMock = mockFetch({ data: updated })
    const api = new KanbanApi(runtimeConfig)

    await expect(api.addTaskLabel("t_1", "backend")).resolves.toEqual(updated)
    expect(calledUrl(fetchMock).pathname).toBe("/api/v1/tasks/t_1/labels")
    expect(JSON.parse(calledInit(fetchMock).body as string)).toEqual({ name: "backend", actor: "desktop-test" })

    vi.unstubAllGlobals()
    const removeFetch = mockFetch({ data: task({ labels: [] }) })
    await api.removeTaskLabel("t_1", "l_backend")
    expect(calledUrl(removeFetch).pathname).toBe("/api/v1/tasks/t_1/labels/l_backend")
    expect(calledInit(removeFetch).method).toBe("DELETE")
  })

  it("rejects hostile task-label response drift", async () => {
    const api = new KanbanApi(runtimeConfig)
    const valid = task({ labels: [{ id: "l_backend", board_id: "b_1", name: "backend", color: null, created_at: 1, updated_at: 1 }] })
    mockFetch({ data: valid, meta: { created_labels: valid.labels } })
    await expect(api.addTaskLabel("t_1", "backend")).resolves.toEqual(valid)
    vi.unstubAllGlobals()
    for (const body of [
      { data: valid, meta: null },
      { data: valid, meta: {} },
      { data: valid, meta: { created_labels: valid.labels, extra: true } },
      { data: valid, meta: { created_labels: {} } },
      { data: valid, meta: { created_labels: [{ ...valid.labels[0], name: undefined }] } },
      { data: valid, meta: { created_labels: [{ ...valid.labels[0], extra: true }] } },
      { data: valid, meta: { created_labels: [{ ...valid.labels[0], created_at: "yesterday" }] } },
      { data: { ...valid, extra: true } },
      { data: valid, extra: true },
    ]) {
      mockFetch(body)
      await expect(api.addTaskLabel("t_1", "backend")).rejects.toMatchObject({ code: "invalid_response" })
      vi.unstubAllGlobals()
    }
    for (const body of [
      { data: valid, extra: true },
      { data: task({ labels: [{ ...valid.labels[0], extra: true } as never] }) },
      { data: task({ labels: [{ ...valid.labels[0], updated_at: "tomorrow" } as never] }) },
    ]) {
      mockFetch(body)
      await expect(api.removeTaskLabel("t_1", "l_backend")).rejects.toMatchObject({ code: "invalid_response" })
      vi.unstubAllGlobals()
    }
  })

  it("requests task label suggestions through the task label route", async () => {
    const suggestion = {
      task_id: "t_1",
      board_id: "b_1",
      selected_labels: [],
      candidates: [],
      coverage: 0,
      coverage_cosine: 0,
      residual_norm: 1,
      needs_new_label: false,
      reason_codes: ["degraded_result", "vector_store_disabled"],
      degraded: true,
      diagnostics: ["vector_store_disabled"],
    }
    const fetchMock = mockFetch({ data: suggestion })
    const api = new KanbanApi(runtimeConfig)

    await expect(
      api.suggestTaskLabels("t_1", {
        limit: 3,
        candidateLimit: 32,
        atomLimit: 80,
        maxSelectedLabels: 4,
        minScore: 0.15,
      }),
    ).resolves.toEqual(suggestion)
    const url = calledUrl(fetchMock)
    expect(url.pathname).toBe("/api/v1/tasks/t_1/labels/suggestions")
    expect(url.searchParams.get("limit")).toBe("3")
    expect(url.searchParams.get("candidate_limit")).toBe("32")
    expect(url.searchParams.get("atom_limit")).toBe("80")
    expect(url.searchParams.get("max_selected_labels")).toBe("4")
    expect(url.searchParams.get("min_score")).toBe("0.15")
  })

  it.each([
    ["missing field", { task_id: "t_1", board_id: "b_1", selected_labels: [], candidates: [], coverage: 0, coverage_cosine: 0, residual_norm: 1, needs_new_label: false, reason_codes: [], degraded: false }],
    ["extra field", { task_id: "t_1", board_id: "b_1", selected_labels: [], candidates: [], coverage: 0, coverage_cosine: 0, residual_norm: 1, needs_new_label: false, reason_codes: [], degraded: false, diagnostics: [], extra: true }],
    ["wrong known field", { task_id: "t_1", board_id: "b_1", selected_labels: [], candidates: [], coverage: "zero", coverage_cosine: 0, residual_norm: 1, needs_new_label: false, reason_codes: [], degraded: false, diagnostics: [] }],
  ])("rejects hostile label suggestion response: %s", async (_case, data) => {
    mockFetch({ data })
    await expect(new KanbanApi(runtimeConfig).suggestTaskLabels("t_1")).rejects.toMatchObject({ code: "invalid_response" })
  })

  it("uses existing ontology HTTP routes for review workbench data and lifecycle actions", async () => {
    const signal = labelOntologySignal({ id: "los_1", target_label_name_snapshot: "cli" })
    const fetchMock = mockFetchSequence([
      { data: [signal], meta: { limit: 25 } },
      { data: [labelOntologyReviewGroup({ key: "cli", label_name: "cli", signal_ids: ["los_1"] })] },
      { data: { signal, observation: labelOntologyObservation({ signals: [signal] }), actions: [] } },
      { data: labelOntologyAction({ id: "loa_confirm", action_type: "confirm", signal_ids: ["los_1"] }) },
      { data: labelAtomExplain({ query: "hash_1" }) },
    ])
    const api = new KanbanApi(runtimeConfig)

    await expect(
      api.listLabelOntologySignals({
        statuses: ["open", "confirmed"],
        kinds: ["false_negative"],
        includeAll: false,
        limit: 25,
      }),
    ).resolves.toEqual([signal])
    expect(new URL(String(fetchMock.mock.calls[0]?.[0])).pathname).toBe("/api/v1/boards/default/label-ontology/signals")
    expect(new URL(String(fetchMock.mock.calls[0]?.[0])).searchParams.getAll("status")).toEqual(["open", "confirmed"])
    expect(new URL(String(fetchMock.mock.calls[0]?.[0])).searchParams.getAll("kind")).toEqual(["false_negative"])

    await api.reviewLabelOntology({ groupBy: "candidate_atom", includeAll: true, limit: 10 })
    expect(new URL(String(fetchMock.mock.calls[1]?.[0])).pathname).toBe("/api/v1/boards/default/label-ontology/review")
    expect(new URL(String(fetchMock.mock.calls[1]?.[0])).searchParams.get("group_by")).toBe("candidate_atom")
    expect(new URL(String(fetchMock.mock.calls[1]?.[0])).searchParams.get("include_all")).toBe("true")

    await api.getLabelOntologySignal("los_1")
    expect(new URL(String(fetchMock.mock.calls[2]?.[0])).pathname).toBe("/api/v1/label-ontology/signals/los_1")

    await api.createLabelOntologyAction({
      actionType: "confirm",
      signalIds: ["los_1"],
      reason: "Reviewed from Desktop",
    })
    expect(new URL(String(fetchMock.mock.calls[3]?.[0])).pathname).toBe("/api/v1/boards/default/label-ontology/actions")
    expect(fetchMock.mock.calls[3]?.[1]?.method).toBe("POST")
    expect(JSON.parse(String(fetchMock.mock.calls[3]?.[1]?.body))).toEqual({
      actor: { name: "desktop-test", type: "user", agent_type: null },
      action_type: "confirm",
      signal_ids: ["los_1"],
      reason: "Reviewed from Desktop",
      superseded_by_signal_id: null,
    })

    await api.explainLabelAtom("hash_1")
    expect(new URL(String(fetchMock.mock.calls[4]?.[0])).pathname).toBe("/api/v1/boards/default/labels/atoms/hash_1/explain")
  })

  it("uses generic signal HTTP routes for signal inbox data", async () => {
    const signal = genericSignal({ id: "sig_1", kind: "agent_cli_friction" })
    const fetchMock = mockFetchSequence([{ data: [signal] }, { data: [signal] }, { data: signal }])
    const api = new KanbanApi(runtimeConfig)

    await expect(
      api.listSignals({
        statuses: ["open", "confirmed"],
        kinds: ["agent_cli_friction", " cli_help_gap "],
        task: " default#123 ",
        includeAll: false,
        limit: 25,
      }),
    ).resolves.toEqual([signal])
    expect(new URL(String(fetchMock.mock.calls[0]?.[0])).pathname).toBe("/api/v1/boards/default/signals")
    expect(new URL(String(fetchMock.mock.calls[0]?.[0])).searchParams.getAll("status")).toEqual(["open", "confirmed"])
    expect(new URL(String(fetchMock.mock.calls[0]?.[0])).searchParams.getAll("kind")).toEqual(["agent_cli_friction", "cli_help_gap"])
    expect(new URL(String(fetchMock.mock.calls[0]?.[0])).searchParams.get("task")).toBe("default#123")

    await api.reviewSignals({ statuses: ["resolved"], includeAll: true, limit: 10 })
    expect(new URL(String(fetchMock.mock.calls[1]?.[0])).pathname).toBe("/api/v1/boards/default/signals/review")
    expect(new URL(String(fetchMock.mock.calls[1]?.[0])).searchParams.get("include_all")).toBe("true")
    expect(new URL(String(fetchMock.mock.calls[1]?.[0])).searchParams.getAll("status")).toEqual(["resolved"])

    await api.getSignal("sig_1")
    expect(new URL(String(fetchMock.mock.calls[2]?.[0])).pathname).toBe("/api/v1/signals/sig_1")
  })

  it("does not expose Desktop helpers for canonical ontology mutations", () => {
    expect(apiSource).toContain("async createLabelOntologyAction")
    expect(apiSource).not.toMatch(
      /\b(?:applyLabelOntologyAtom|upsertLabelSemantics|deleteLabelSemantics|revertLabelOntologyMutation|validateLabelOntologyAction)\b/,
    )
    expect(apiSource).not.toMatch(
      /\/api\/v1\/boards\/\$\{this\.board\}\/label-ontology\/(?:apply|semantics|revert|validate)/,
    )
  })

  it("lists active boards through the boards endpoint", async () => {
    const boards = [
      board({ id: "b_default", slug: "default", name: "Default" }),
      board({ id: "b_ops", slug: "ops", name: "Ops" }),
    ]
    const fetchMock = mockFetch({ data: boards })
    const api = new KanbanApi(runtimeConfig)

    const result = await api.listBoards()

    expect(result).toEqual(boards)
    expect(calledUrl(fetchMock).pathname).toBe("/api/v1/boards")
    expect(calledUrl(fetchMock).searchParams.get("include_archived")).toBe("false")
  })

  it("can include archived boards when listing boards", async () => {
    const fetchMock = mockFetch({ data: [board({ archived_at: 10 })] })
    const api = new KanbanApi(runtimeConfig)

    await api.listBoards({ includeArchived: true })

    expect(calledUrl(fetchMock).searchParams.get("include_archived")).toBe("true")
  })

  it("rejects malformed board list envelopes before React consumes them", async () => {
    mockFetch({ data: { not: "an array" } })
    const api = new KanbanApi(runtimeConfig)

    await expect(api.listBoards()).rejects.toMatchObject({
      code: "invalid_response",
      message: "boards response data must be an array",
    } satisfies Partial<ApiError>)
  })

  it("rejects board list rows with missing, mistyped, or unknown contract fields", async () => {
    const api = new KanbanApi(runtimeConfig)

    mockFetch({ data: [board({ name: undefined as unknown as string })] })
    await expect(api.listBoards()).rejects.toMatchObject({ code: "invalid_response" })

    mockFetch({ data: [board({ archived_at: "later" as unknown as number })] })
    await expect(api.listBoards()).rejects.toMatchObject({ code: "invalid_response" })

    mockFetch({ data: [{ ...board(), extra: true }] })
    await expect(api.listBoards()).rejects.toMatchObject({ code: "invalid_response" })
  })

  it("rejects board list outer extras and scalar or null data", async () => {
    const api = new KanbanApi(runtimeConfig)

    for (const envelope of [
      { data: [], extra: true },
      { data: 1 },
      { data: null },
    ]) {
      mockFetch(envelope)
      await expect(api.listBoards()).rejects.toMatchObject({ code: "invalid_response" })
    }
  })

  it("creates, gets, and archives boards through exact production transports", async () => {
    const created = board({ id: "b_ops", slug: "ops", name: "Operations", description: "Runbooks" })
    const archived = board({ ...created, archived_at: 30, updated_at: 30 })
    const fetchMock = mockFetchSequence([
      { data: created },
      { data: created },
      { data: archived },
    ])
    const api = new KanbanApi(runtimeConfig, { locale: "zh-CN" })

    await expect(api.createBoard({ slug: "ops", name: "Operations", description: "Runbooks" })).resolves.toEqual(created)
    await expect(api.getBoard("ops / on-call")).resolves.toEqual(created)
    await expect(api.archiveBoard("ops / on-call")).resolves.toEqual(archived)

    const [createUrl, createInit] = fetchMock.mock.calls[0]!
    expect(new URL(String(createUrl)).pathname).toBe("/api/v1/boards")
    expect(createInit?.method).toBe("POST")
    expect(createInit?.headers).toMatchObject({
      "Accept-Language": "zh-CN",
      "Content-Type": "application/json",
      "X-KB-Actor": "desktop-test",
    })
    expect(JSON.parse(String(createInit?.body))).toEqual({
      slug: "ops",
      name: "Operations",
      description: "Runbooks",
      actor: "desktop-test",
    })

    const [getUrl, getInit] = fetchMock.mock.calls[1]!
    expect(new URL(String(getUrl)).pathname).toBe("/api/v1/boards/ops%20%2F%20on-call")
    expect(getInit?.method).toBe("GET")
    expect(getInit?.body).toBeUndefined()
    expect(getInit?.headers).toEqual({ "Accept-Language": "zh-CN" })

    const [archiveUrl, archiveInit] = fetchMock.mock.calls[2]!
    expect(new URL(String(archiveUrl)).pathname).toBe("/api/v1/boards/ops%20%2F%20on-call/archive")
    expect(archiveInit?.method).toBe("POST")
    expect(archiveInit?.headers).toMatchObject({
      "Accept-Language": "zh-CN",
      "Content-Type": "application/json",
      "X-KB-Actor": "desktop-test",
    })
    expect(JSON.parse(String(archiveInit?.body))).toEqual({ actor: "desktop-test" })
  })

  it("rejects hostile endpoint-specific create, get, and archive envelopes", async () => {
    const api = new KanbanApi(runtimeConfig)
    const cases = [
      [{ data: { ...board(), extra: true } }, () => api.createBoard({ slug: "ops", name: "Ops" })],
      [{ data: 7 }, () => api.getBoard("ops")],
      [{ data: null }, () => api.archiveBoard("ops")],
    ] as const

    for (const [envelope, call] of cases) {
      mockFetch(envelope)
      await expect(call()).rejects.toMatchObject({ code: "invalid_response" })
    }
  })

  it("uses backend-shaped maintenance and status envelopes", async () => {
    const searchStatusEnvelope = {
      backend: "sqlite",
      derived_index: false,
      stale: false,
      index_version: null,
      last_event_id: null,
      index_lag_events: 0,
      message: "SQLite fallback search is active",
    } satisfies SearchIndexStatus
    const fetchMock = mockFetchSequence([
      {
        data: {
          board_id: "b_1",
          generated_at: 10,
          status_counts: [{ status: "ready", count: 3 }],
          stale_claims: [
            {
              task_id: "t_stale",
              seq: 7,
              title: "stale worker",
              claim_owner: "dispatcher",
              claim_expires_at: 8,
              last_heartbeat_at: 5,
              current_run_id: "r_1",
              retry_count: 1,
              max_retries: 3,
            },
          ],
          blocked_reasons: [{ reason: "waiting", count: 2 }],
        },
      },
      {
        data: searchStatusEnvelope,
      },
      {
        data: {
          ok: true,
          integrity_check: "ok",
          migration_version: 1,
          user_version: 0,
          expired_running_tasks: 0,
          running_tasks_without_active_run: 0,
          orphan_running_runs: 0,
          dependency_cycles: 0,
          archived_dependency_edges: 0,
          missing_run_logs: 0,
          suspicious_run_log_paths: 0,
          executable_dependency_violations: 0,
          executable_spec_violations: 0,
          executable_schedule_violations: 0,
          outbox_pending: 0,
          outbox_running: 0,
          outbox_failed: 0,
          unplanned_active_tasks: 0,
          active_parents_with_incomplete_required_steps: 0,
          derived_dirty_stores: 0,
          derived_error_stores: 0,
          derived_stores: [],
          consistency_errors: 0,
          consistency_warnings: 0,
          consistency_issues: [],
          ontology_ledger_errors: 0,
          ontology_ledger_warnings: 0,
          ontology_ledger_issues: [],
        },
      },
      {
        data: {
          busy: 0,
          log_frames: 4,
          checkpointed_frames: 4,
        },
      },
    ])
    const api = new KanbanApi(runtimeConfig)

    const stats = await api.stats()
    expect(stats.status_counts).toEqual([{ status: "ready", count: 3 }])
    expect(stats.stale_claims[0].task_id).toBe("t_stale")
    expect(stats.blocked_reasons).toEqual([{ reason: "waiting", count: 2 }])
    expect(calledUrl(fetchMock).pathname).toBe("/api/v1/stats")
    expect(calledUrl(fetchMock).searchParams.get("board")).toBe("default")

    const searchStatus = await api.searchStatus()
    expect(searchStatus).toEqual(searchStatusEnvelope)
    expect(searchStatus.derived_index).toBe(false)
    expect(searchStatus.message).toBe("SQLite fallback search is active")
    expect(fetchMock.mock.calls[1] ? new URL(String(fetchMock.mock.calls[1][0])).pathname : "").toBe("/api/v1/search/status")

    const doctor = await api.doctor()
    expect(doctor.integrity_check).toBe("ok")
    expect(fetchMock.mock.calls[2] ? new URL(String(fetchMock.mock.calls[2][0])).pathname : "").toBe("/api/v1/maintenance/doctor")
    expect(fetchMock.mock.calls[2]?.[1]?.method).toBe("POST")
    expect(fetchMock.mock.calls[2]?.[1]?.body).toBeUndefined()

    const checkpoint = await api.checkpoint()
    expect(checkpoint).toEqual({ busy: 0, log_frames: 4, checkpointed_frames: 4 })
    expect(fetchMock.mock.calls[3] ? new URL(String(fetchMock.mock.calls[3][0])).pathname : "").toBe("/api/v1/maintenance/checkpoint")
    expect(fetchMock.mock.calls[3]?.[1]?.method).toBe("POST")
  })
  it("strictly consumes maintenance contract fixtures and rejects hostile drift", async () => {
    const doctorFixture = JSON.parse(readFileSync(new URL("../../../../schemas/fixtures/api/doctor-response.v1.valid.json", import.meta.url), "utf8"))
    const checkpointFixture = JSON.parse(readFileSync(new URL("../../../../schemas/fixtures/api/checkpoint-response.v1.valid.json", import.meta.url), "utf8"))
    const api = new KanbanApi(runtimeConfig)

    mockFetch(doctorFixture)
    expect((await api.doctor()).derived_stores[0]?.store_name).toBe("fixture_store")
    vi.unstubAllGlobals()

    mockFetch(checkpointFixture)
    expect(await api.checkpoint()).toEqual(checkpointFixture.data)
    vi.unstubAllGlobals()

    mockFetch({ ...doctorFixture, extra: true })
    await expect(api.doctor()).rejects.toMatchObject({ code: "invalid_response" })
    vi.unstubAllGlobals()

    const nestedDrift = structuredClone(checkpointFixture)
    nestedDrift.data.unexpected = true
    mockFetch(nestedDrift)
    await expect(api.checkpoint()).rejects.toMatchObject({ code: "invalid_response" })
    vi.unstubAllGlobals()

    const missingKey = structuredClone(doctorFixture)
    delete missingKey.data.ok
    mockFetch(missingKey)
    await expect(api.doctor()).rejects.toMatchObject({ code: "invalid_response" })
    vi.unstubAllGlobals()

    const wrongNestedType = structuredClone(doctorFixture)
    wrongNestedType.data.derived_stores[0].schema_version = "1"
    mockFetch(wrongNestedType)
    await expect(api.doctor()).rejects.toMatchObject({ code: "invalid_response" })
  })

  it("deletes parent dependencies through the child scoped endpoint", async () => {
    const fetchMock = mockFetch({ data: { parents: [], children: [] } })
    const api = new KanbanApi(runtimeConfig)

    await api.removeDependency("t_child", "t_parent")

    expect(calledUrl(fetchMock).pathname).toBe("/api/v1/tasks/t_child/dependencies/t_parent")
    expect(calledInit(fetchMock).method).toBe("DELETE")
  })

  it("uses step routes and includes the desktop actor", async () => {
    const child = task({ id: "t_child", title: "Child" })
    const steps = {
      task_id: "t_parent",
      steps: [
        {
          id: "step_1",
          parent_task_id: "t_parent",
          title: "Review child",
          body: "child context",
          linked_task: child,
          position: 2048,
          required: true,
          status: "todo",
          resolution_note: null,
          resolved_by: null,
          resolved_at: null,
          created_by: "desktop-test",
          created_at: 1,
          updated_by: "desktop-test",
          updated_at: 1,
        },
      ],
      execution_plan: {
        board_id: "b_1",
        task_id: "t_parent",
        state: "planned",
        reason: null,
        updated_by: "system",
        updated_at: 0,
      },
    }
    const fetchMock = mockFetch({ data: steps })
    const api = new KanbanApi(runtimeConfig)

    await expect(
      api.createStep("t_parent", {
        title: "Review child",
        body: "child context",
        linked_task_ref: "#123",
        required: true,
        position: 2048,
      }),
    ).resolves.toEqual(steps)
    expect(calledUrl(fetchMock).pathname).toBe("/api/v1/tasks/t_parent/steps")
    expect(calledInit(fetchMock).method).toBe("POST")
    expect(JSON.parse(calledInit(fetchMock).body as string)).toEqual({
      title: "Review child",
      body: "child context",
      linked_task_ref: "#123",
      required: true,
      position: 2048,
      actor: "desktop-test",
    })

    vi.unstubAllGlobals()
    const updateFetch = mockFetch({ data: steps })
    await api.updateStep("t_parent", "step_1", { position: 4096, required: false, unlink_task: true })
    expect(calledUrl(updateFetch).pathname).toBe("/api/v1/tasks/t_parent/steps/step_1")
    expect(calledInit(updateFetch).method).toBe("PATCH")
    expect(JSON.parse(calledInit(updateFetch).body as string)).toEqual({
      position: 4096,
      required: false,
      unlink_task: true,
      actor: "desktop-test",
    })

    vi.unstubAllGlobals()
    const doneFetch = mockFetch({ data: steps })
    await api.completeStep("t_parent", "step_1", "done")
    expect(calledUrl(doneFetch).pathname).toBe("/api/v1/tasks/t_parent/steps/step_1/done")
    expect(JSON.parse(calledInit(doneFetch).body as string)).toEqual({ note: "done", actor: "desktop-test" })

    vi.unstubAllGlobals()
    const skipFetch = mockFetch({ data: steps })
    await api.skipStep("t_parent", "step_1", "not needed")
    expect(calledUrl(skipFetch).pathname).toBe("/api/v1/tasks/t_parent/steps/step_1/skip")
    expect(JSON.parse(calledInit(skipFetch).body as string)).toEqual({ reason: "not needed", actor: "desktop-test" })

    vi.unstubAllGlobals()
    const reopenFetch = mockFetch({ data: steps })
    await api.reopenStep("t_parent", "step_1", "redo")
    expect(calledUrl(reopenFetch).pathname).toBe("/api/v1/tasks/t_parent/steps/step_1/reopen")
    expect(JSON.parse(calledInit(reopenFetch).body as string)).toEqual({ reason: "redo", actor: "desktop-test" })

    vi.unstubAllGlobals()
    const removeFetch = mockFetch({ data: { ...steps, steps: [] } })
    await api.removeStep("t_parent", "step_1")
    expect(calledUrl(removeFetch).pathname).toBe("/api/v1/tasks/t_parent/steps/step_1")
    expect(calledInit(removeFetch).method).toBe("DELETE")
  })

  it("uses execution plan and task graph routes", async () => {
    const plan = {
      board_id: "b_1",
      task_id: "t_parent",
      state: "not_required",
      reason: "small cleanup",
      updated_by: "desktop-test",
      updated_at: 2,
    }
    const fetchMock = mockFetchSequence([
      { data: plan },
      { data: { center_task_id: "t_parent", nodes: [], edges: [], meta: { depth: 1, generated_at: 1, truncated: false, node_count: 0, edge_count: 0 } } },
      { data: { nodes: [], edges: [], meta: { context_depth: 1, generated_at: 1, truncated: false, node_count: 0, edge_count: 0 } } },
    ])
    const api = new KanbanApi(runtimeConfig)

    await expect(api.markExecutionPlanNotRequired("t_parent", "small cleanup")).resolves.toEqual(plan)
    expect(new URL(String(fetchMock.mock.calls[0]?.[0])).pathname).toBe("/api/v1/tasks/t_parent/execution-plan/not-required")
    expect(fetchMock.mock.calls[0]?.[1]?.method).toBe("POST")
    expect(JSON.parse(String(fetchMock.mock.calls[0]?.[1]?.body))).toEqual({
      reason: "small cleanup",
      actor: "desktop-test",
    })

    await api.getTaskNeighborhood("t_parent", { limitNodes: 20 })
    const neighborhoodUrl = new URL(String(fetchMock.mock.calls[1]?.[0]))
    expect(neighborhoodUrl.pathname).toBe("/api/v1/tasks/t_parent/neighborhood")
    expect(neighborhoodUrl.searchParams.get("depth")).toBe("1")
    expect(neighborhoodUrl.searchParams.get("limit_nodes")).toBe("20")

    await api.getBoardTaskMap("default", { includeDoneContext: true, includeArchivedContext: false, hideIsolated: true })
    const mapUrl = new URL(String(fetchMock.mock.calls[2]?.[0]))
    expect(mapUrl.pathname).toBe("/api/v1/boards/default/task-map")
    expect(mapUrl.searchParams.get("active_only")).toBe("true")
    expect(mapUrl.searchParams.get("context_depth")).toBe("1")
    expect(mapUrl.searchParams.get("include_done_context")).toBe("true")
    expect(mapUrl.searchParams.get("include_archived_context")).toBe("false")
    expect(mapUrl.searchParams.get("hide_isolated")).toBe("true")
  })

})

describe("loadRuntimeConfig web mode", () => {
  afterEach(() => {
    vi.unstubAllEnvs()
    vi.unstubAllGlobals()
  })

  it("uses the local Vite API proxy by default in web dev mode", async () => {
    vi.stubEnv("VITE_KB_API_BASE_URL", "")

    await expect(loadRuntimeConfig()).resolves.toEqual({
      apiBaseUrl: "/__kb_api__",
      dbPath: "local kanban serve",
      actor: "desktop-dev",
      board: "kanban-tool",
    })
  })

  it("requires an explicit API base URL outside Tauri in production web mode", async () => {
    vi.stubEnv("DEV", false)
    vi.stubEnv("VITE_KB_API_BASE_URL", "")

    await expect(loadRuntimeConfig()).rejects.toThrow("VITE_KB_API_BASE_URL")
  })

  it("uses the explicit API base URL and optional dev database label outside Tauri", async () => {
    vi.stubEnv("VITE_KB_API_BASE_URL", "/__kb_api__")
    vi.stubEnv("VITE_KB_DB_PATH", "/tmp/current-kanban.db")
    vi.stubEnv("VITE_KB_ACTOR", "web-test")
    vi.stubEnv("VITE_KB_BOARD", "ops")

    await expect(loadRuntimeConfig()).resolves.toEqual({
      apiBaseUrl: "/__kb_api__",
      dbPath: "/tmp/current-kanban.db",
      actor: "web-test",
      board: "ops",
    })
  })
})

function mockFetch(envelope: unknown) {
  const fetchMock = vi.fn(async (input: string, init?: RequestInit) => {
    void input
    void init
    return {
      ok: true,
      status: 200,
      statusText: "OK",
      text: async () => JSON.stringify(envelope),
    }
  })
  vi.stubGlobal("fetch", fetchMock)
  return fetchMock
}

function mockFetchSequence(envelopes: unknown[]) {
  const fetchMock = vi.fn(async (input: string, init?: RequestInit) => {
    void input
    void init
    const envelope = envelopes.shift()
    expect(envelope).toBeDefined()
    return {
      ok: true,
      status: 200,
      statusText: "OK",
      text: async () => JSON.stringify(envelope),
    }
  })
  vi.stubGlobal("fetch", fetchMock)
  return fetchMock
}

function calledUrl(fetchMock: ReturnType<typeof mockFetch>) {
  const url = fetchMock.mock.calls[0]?.[0]
  expect(url).toBeDefined()
  return new URL(url)
}

function calledInit(fetchMock: ReturnType<typeof mockFetch>) {
  const init = fetchMock.mock.calls[0]?.[1]
  expect(init).toBeDefined()
  return init!
}

function eventRecord(overrides: Partial<import("./api").EventRecord> = {}): import("./api").EventRecord {
  return {
    id: 1,
    event_id: "e_1",
    board_id: "b_1",
    task_id: "t_1",
    run_id: null,
    kind: "task.created",
    actor: "seed",
    payload: {},
    created_at: 1,
    ...overrides,
  }
}

function board(overrides: Partial<Board> = {}): Board {
  return {
    id: "b_1",
    slug: "default",
    name: "Default",
    description: null,
    created_at: 1,
    updated_at: 1,
    archived_at: null,
    ...overrides,
  }
}

function task(overrides: Partial<Task> = {}): Task {
  return {
    id: "t_1",
    board_id: "b_1",
    board_slug: "default",
    ref: "default#1",
    seq: 1,
    title: "Task",
    description: null,
    status: "ready",
    status_reason: null,
    assignee: null,
    priority: 0,
    position: 0,
    scheduled_at: null,
    due_at: null,
    created_by: "seed",
    created_at: 1,
    updated_at: 1,
    started_at: null,
    completed_at: null,
    archived_at: null,
    claim_owner: null,
    claim_expires_at: null,
    last_heartbeat_at: null,
    current_run_id: null,
    retry_count: 0,
    max_retries: null,
    result_summary: null,
    result: null,
    metadata: {},
    lock_version: 0,
    dependency_blocked: false,
    unfinished_parent_count: 0,
    execution_plan_state: "unplanned",
    required_step_count: 0,
    completed_required_step_count: 0,
    optional_step_count: 0,
    labels: [],
    ...overrides,
  }
}

function labelOntologySignal(overrides: Partial<import("./api").LabelOntologySignalRecord> = {}): import("./api").LabelOntologySignalRecord {
  return {
    id: "los_1",
    observation_id: "loo_1",
    board_id: "b_1",
    kind: "false_negative",
    status: "open",
    target_label_id: "lab_cli",
    target_label_name_snapshot: "cli",
    related_labels_json: "[]",
    proposed_action: "add_positive_atom",
    candidate_atom_polarity: "positive",
    candidate_atom_kind: "applies_when",
    candidate_text: "touches CLI behavior",
    candidate_content_hash: "hash_1",
    proposed_label_name: null,
    proposed_label_name_normalized: null,
    proposal_json: "{}",
    agent_selected: true,
    suggest_state: "absent",
    suggest_score: 0.12,
    suggest_rank: 4,
    final_selected: true,
    rationale: "Review rationale",
    confidence: 0.9,
    signal_key: "signal-key",
    superseded_by_signal_id: null,
    status_reason: null,
    created_at: 1,
    updated_at: 1,
    reviewed_at: null,
    closed_at: null,
    ...overrides,
  }
}

function genericSignal(overrides: Partial<import("./api").SignalRecord> = {}): import("./api").SignalRecord {
  return {
    id: "sig_1",
    board_id: "b_1",
    observation_id: "obs_1",
    kind: "agent_cli_friction",
    title: "CLI friction",
    summary: "Agent observed a CLI argument mismatch.",
    severity: "info",
    status: "open",
    dedupe_key: "dedupe-cli-friction",
    superseded_by_signal_id: null,
    reviewed_by: null,
    reviewed_at: null,
    review_reason: null,
    created_at: 1,
    updated_at: 2,
    observation: {
      id: "obs_1",
      board_id: "b_1",
      task_id: "t_1",
      task_ref_snapshot: "default#123",
      run_id: null,
      comment_id: null,
      actor: "codex",
      agent_type: "codex",
      source: "api-test",
      evidence_json: "{}",
      created_at: 1,
    },
    ...overrides,
  }
}

function labelOntologyObservation(
  overrides: Partial<import("./api").LabelOntologyObservationRecord> = {},
): import("./api").LabelOntologyObservationRecord {
  return {
    id: "loo_1",
    board_id: "b_1",
    task_id: "t_1",
    task_ref_snapshot: "default#1",
    task_snapshot_json: "{}",
    suggest_input_hash: "input-hash",
    agent_candidates_json: "[]",
    suggestion_snapshot_json: "{}",
    final_decision_json: "{}",
    suggest_coverage: 0.6,
    suggest_coverage_cosine: 0.7,
    suggest_residual_norm: 0.4,
    suggest_needs_new_label: false,
    suggest_degraded: false,
    diagnostics_json: "[]",
    capture_fingerprint: "fingerprint",
    created_by: "desktop-test",
    created_by_type: "user",
    agent_type: null,
    created_at: 1,
    signals: [],
    ...overrides,
  }
}

function labelOntologyAction(
  overrides: Partial<import("./api").LabelOntologyActionRecord> = {},
): import("./api").LabelOntologyActionRecord {
  return {
    id: "loa_1",
    board_id: "b_1",
    parent_action_id: null,
    action_type: "confirm",
    reason: "Reviewed",
    target_label_id: null,
    result_label_id: null,
    result_atom_id: null,
    result_atom_content_hash: null,
    result_proposal_id: null,
    canonical_before_hash: null,
    canonical_after_hash: null,
    change_json: "{}",
    validation_requirement: "none",
    validation_status: "not_required",
    validation_effective_outcome: "not_required",
    validation_latest_attempt_id: null,
    validation_json: "{}",
    created_by: "desktop-test",
    created_by_type: "user",
    agent_type: null,
    created_at: 1,
    signal_ids: [],
    ...overrides,
  }
}

function labelOntologyReviewGroup(
  overrides: Partial<import("./api").LabelOntologyReviewGroup> = {},
): import("./api").LabelOntologyReviewGroup {
  return {
    group_by: "label",
    key: "lab_cli",
    label_id: "lab_cli",
    label_name: "cli",
    candidate_atom_polarity: "positive",
    candidate_atom_kind: "applies_when",
    candidate_text: "touches CLI behavior",
    candidate_content_hash: "hash_1",
    proposed_label_name: null,
    proposed_label_name_normalized: null,
    cluster_key: null,
    cluster_reason: null,
    task_count: 1,
    signal_count: 1,
    open_count: 1,
    confirmed_count: 0,
    resolved_count: 0,
    rejected_count: 0,
    superseded_count: 0,
    degraded_count: 0,
    average_score: 0.12,
    median_score: 0.12,
    oldest_signal_at: 1,
    latest_signal_at: 1,
    sample_task_refs: ["default#1"],
    signal_ids: ["los_1"],
    action_count: 0,
    action_ids: [],
    proposal_ids: [],
    labels: [{ id: "lab_cli", name: "cli" }],
    candidate_atom_variants: [],
    ...overrides,
  }
}

function labelAtomExplain(overrides: Partial<import("./api").LabelAtomExplainRecord> = {}): import("./api").LabelAtomExplainRecord {
  return {
    query: "hash_1",
    atom: null,
    current_semantics: null,
    provenance_actions: [],
    supporting_signals: [],
    validation_history: [],
    legacy_untracked: false,
    legacy_reason: null,
    ...overrides,
  }
}

  it("rejects missing or malformed C2b task-read pagination metadata", async () => {
    const api = new KanbanApi(runtimeConfig)
    const malformed = [
      { data: [task({ id: "t_missing_meta" })] },
      { data: [task({ id: "t_wrong_total" })], meta: { limit: 10, offset: 0, total: "1" } },
      {
        data: { statuses: [{ status: "ready", tasks: [], page: { limit: 10, offset: 0 } }] },
        meta: { limit: 10, offset: 0 },
      },
      {
        data: { statuses: [{ status: "ready", tasks: [], page: { limit: "10", offset: 0, total: 0 } }] },
        meta: { limit: 10, offset: "0" },
      },
    ]

    for (const response of malformed.slice(0, 2)) {
      mockFetch(response)
      await expect(api.listTasks({ limit: 10 })).rejects.toMatchObject({ code: "invalid_response" } satisfies Partial<ApiError>)
      vi.unstubAllGlobals()
    }
    for (const response of malformed.slice(2)) {
      mockFetch(response)
      await expect(api.listTasksByStatus({ statuses: ["ready"], limit: 10 })).rejects.toMatchObject({ code: "invalid_response" } satisfies Partial<ApiError>)
      vi.unstubAllGlobals()
    }
  })
