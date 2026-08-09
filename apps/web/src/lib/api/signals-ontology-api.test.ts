import { describe, expect, test, vi } from "vitest"

import type { WebRuntimeConfig } from "../runtime"
import { asCanonicalBoardId } from "../sync/contracts"
import type { HttpTransportResponse } from "./http-transport"
import { createSignalsOntologyReadApi, SignalsOntologyReadError, type SignalsOntologyReadTransport } from "./signals-ontology-read-model"

const runtime = {
  apiBaseUrl: "/__kb_api__",
  webBasePath: "/app/",
  actor: "codex",
  defaultBoard: "default",
  serverVersion: "3.0.0",
  protocolVersion: "v1",
  webBuildId: "dev",
} satisfies WebRuntimeConfig

const signal = {
  id: "sig_1",
  board_id: "b_1",
  observation_id: "obs_1",
  kind: "agent_cli_friction",
  title: "CLI friction",
  summary: "A command was rejected.",
  severity: "info",
  status: "open",
  dedupe_key: null,
  superseded_by_signal_id: null,
  reviewed_by: null,
  reviewed_at: null,
  review_reason: null,
  created_at: 1,
  updated_at: 1,
  observation: {
    id: "obs_1",
    board_id: "b_1",
    task_id: null,
    task_ref_snapshot: null,
    run_id: null,
    comment_id: null,
    actor: "codex",
    agent_type: null,
    source: "test",
    evidence: { command: "kanban" },
    created_at: 1,
  },
} as const

const board = {
  id: "b_1",
  slug: "team-one",
  name: "Team One",
  description: null,
  created_at: 1,
  updated_at: 1,
  archived_at: null,
} as const

const response = <T,>(payload: T): HttpTransportResponse => ({ payload, bytes: 10 })

describe("signals and ontology API seam", () => {
  test("validates generated query/path contracts and keeps board route encoded", async () => {
    const transport: SignalsOntologyReadTransport = {
      get: vi.fn(async (path: string): Promise<HttpTransportResponse> => {
        if (path.startsWith("/api/v1/boards?")) return response({ data: [board] })
        if (path.includes("signals/review")) return response({ data: [signal], meta: { include_all: false, limit: 100 } })
        return response(signal)
      }),
    }
    const api = createSignalsOntologyReadApi(runtime, {
      board: "b_1",
      transport,
    })

    const rows = await api.reviewSignals({ statuses: ["open"], kinds: ["agent_cli_friction"], task: "team/one#1", includeAll: false, limit: 100 })

    expect(rows).toHaveLength(1)
    expect(transport.get).toHaveBeenCalledWith(
      "/api/v1/boards/b_1/signals/review?status=open&kind=agent_cli_friction&task_ref=team%2Fone%231&include_all=false&limit=100",
      undefined,
    )
  })

  test("rejects malformed response payloads at the generated validator boundary", async () => {
    const transport: SignalsOntologyReadTransport = {
      get: vi.fn(async (): Promise<HttpTransportResponse> => response({ data: [{ id: "sig_1" }] })),
    }
    const api = createSignalsOntologyReadApi(runtime, {
      board: "default",
      identity: { selector: "default", canonicalBoardId: asCanonicalBoardId("b_1"), slug: "default", name: "Default" },
      transport,
    })

    await expect(api.reviewSignals()).rejects.toMatchObject({ name: SignalsOntologyReadError.name, contractId: "api.review-signals.response" })
  })

  test("resolves a slug once, then uses canonical board IDs in cache keys and ontology queries", async () => {
    const transport: SignalsOntologyReadTransport = {
      get: vi.fn(async (path: string): Promise<HttpTransportResponse> => {
        if (path.startsWith("/api/v1/boards?")) return response({ data: [board] })
        if (path.startsWith("/api/v1/boards/b_1/label-ontology/signals?")) return response({ data: [], meta: { include_all: false, limit: 100 } })
        return response({ data: [], meta: { group_by: "label", include_all: false, limit: 100 } })
      }),
    }
    const api = createSignalsOntologyReadApi(runtime, { board: "team-one", transport })

    expect(api.cacheKey).toContain(":team-one")
    await expect(api.listLabelOntologySignals({ statuses: ["open", "confirmed"], includeAll: false })).resolves.toEqual([])
    expect(api.cacheKey).toContain(":b_1")
    expect(transport.get).toHaveBeenCalledWith(
      "/api/v1/boards/b_1/label-ontology/signals?status=open&status=confirmed&include_all=false&limit=100",
      undefined,
    )
  })

  test("rejects a response that crosses the resolved board scope", async () => {
    const transport: SignalsOntologyReadTransport = {
      get: vi.fn(async (path: string): Promise<HttpTransportResponse> => {
        if (path.includes("signals/review")) {
          return response({ data: [{ ...signal, board_id: "b_other", observation: { ...signal.observation, board_id: "b_other" } }], meta: { include_all: false, limit: 100 } })
        }
        return response({ data: [board] })
      }),
    }
    const api = createSignalsOntologyReadApi(runtime, { board: "team-one", transport })

    await expect(api.reviewSignals()).rejects.toMatchObject({ name: SignalsOntologyReadError.name, kind: "board_scope" })
  })

  test("preserves include_all on the canonical review endpoint", async () => {
    const transport: SignalsOntologyReadTransport = {
      get: vi.fn(async (path: string): Promise<HttpTransportResponse> => {
        if (path.includes("signals/review")) return response({ data: [signal], meta: { include_all: true, limit: 1 } })
        return response({ data: [board] })
      }),
    }
    const api = createSignalsOntologyReadApi(runtime, { board: "b_1", transport })

    await expect(api.reviewSignals({ includeAll: true, limit: 1 })).resolves.toHaveLength(1)
    expect(transport.get).toHaveBeenCalledWith(
      "/api/v1/boards/b_1/signals/review?include_all=true&limit=1",
      undefined,
    )
  })

  test("fails closed for malformed injected identity and out-of-range limits", async () => {
    const transport: SignalsOntologyReadTransport = { get: vi.fn(async () => response({ data: [] })) }
    expect(() => createSignalsOntologyReadApi(runtime, {
      board: "default",
      transport,
      identity: { selector: "other", canonicalBoardId: asCanonicalBoardId("b_1"), slug: "default", name: "Default" },
    })).toThrowError(SignalsOntologyReadError)
    expect(() => createSignalsOntologyReadApi(runtime, {
      board: "default",
      transport,
      identity: { selector: "default", canonicalBoardId: asCanonicalBoardId("bad"), slug: "default", name: "Default" },
    })).toThrowError(SignalsOntologyReadError)

    const api = createSignalsOntologyReadApi(runtime, {
      board: "default",
      transport,
      identity: { selector: "default", canonicalBoardId: asCanonicalBoardId("b_1"), slug: "default", name: "Default" },
    })
    await expect(api.reviewSignals({ limit: 0 })).rejects.toMatchObject({ kind: "invalid_response" })
  })

  test("posts only supported lifecycle actions with a stable retry key and board scope", async () => {
    const action = {
      id: "loa_1",
      board_id: "b_1",
      parent_action_id: null,
      action_type: "confirm",
      reason: "keep this signal",
      target_label_id: null,
      result_label_id: null,
      result_atom_id: null,
      result_atom_content_hash: null,
      result_proposal_id: null,
      canonical_before_hash: null,
      canonical_after_hash: null,
      change: {},
      validation_requirement: "none",
      validation_status: "not_required",
      validation_effective_outcome: "not_required",
      validation_latest_attempt_id: null,
      validation: {},
      created_by: "codex",
      created_by_type: "user",
      agent_type: null,
      created_at: 1,
      signal_ids: ["los_1"],
    } as const
    const post = vi.fn(async (): Promise<HttpTransportResponse> => response({ data: action }))
    const transport: SignalsOntologyReadTransport = { get: vi.fn(), post }
    const api = createSignalsOntologyReadApi(runtime, {
      board: "default",
      identity: { selector: "default", canonicalBoardId: asCanonicalBoardId("b_1"), slug: "default", name: "Default" },
      transport,
    })

    await expect(api.createLabelOntologyLifecycleAction("confirm", "los_1", "keep this signal")).resolves.toEqual(action)
    await expect(api.createLabelOntologyLifecycleAction("confirm", "los_1", "keep this signal")).resolves.toEqual(action)
    expect(post).toHaveBeenCalledTimes(2)
    expect(post.mock.calls[0]?.[0]).toBe("/api/v1/boards/b_1/label-ontology/actions")
    const firstBody = post.mock.calls[0]?.[1] as { idempotency_key?: string }
    const secondBody = post.mock.calls[1]?.[1] as { idempotency_key?: string }
    expect(post.mock.calls[0]?.[1]).toMatchObject({
      actor: { name: "codex", type: "user", agent_type: null },
      action_type: "confirm",
      signal_ids: ["los_1"],
      reason: "keep this signal",
      idempotency_key: firstBody.idempotency_key,
    })
    expect(firstBody.idempotency_key).toBeTruthy()
    expect(firstBody.idempotency_key).toBe(secondBody.idempotency_key)
  })

  test("rejects lifecycle responses outside the resolved board", async () => {
    const transport: SignalsOntologyReadTransport = {
      get: vi.fn(),
      post: vi.fn(async (): Promise<HttpTransportResponse> => response({
        data: {
          id: "loa_other",
          board_id: "b_other",
          parent_action_id: null,
          action_type: "confirm",
          reason: "scope",
          target_label_id: null,
          result_label_id: null,
          result_atom_id: null,
          result_atom_content_hash: null,
          result_proposal_id: null,
          canonical_before_hash: null,
          canonical_after_hash: null,
          change: {},
          validation_requirement: "none",
          validation_status: "not_required",
          validation_effective_outcome: "not_required",
          validation_latest_attempt_id: null,
          validation: {},
          created_by: "codex",
          created_by_type: "user",
          agent_type: null,
          created_at: 1,
          signal_ids: ["los_1"],
        },
      })),
    }
    const api = createSignalsOntologyReadApi(runtime, {
      board: "default",
      identity: { selector: "default", canonicalBoardId: asCanonicalBoardId("b_1"), slug: "default", name: "Default" },
      transport,
    })
    await expect(api.createLabelOntologyLifecycleAction("confirm", "los_1", "scope")).rejects.toMatchObject({ kind: "board_scope" })
  })
})
