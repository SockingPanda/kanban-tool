import { describe, expect, test, vi } from "vitest"

import type { WebRuntimeConfig } from "../runtime"
import type { HttpTransportResponse } from "./http-transport"
import { createSignalsOntologyReadApi, type SignalsOntologyReadTransport } from "./signals-ontology-read-model"

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

const response = <T,>(payload: T): HttpTransportResponse => ({ payload, bytes: 10 })

describe("signals and ontology API seam", () => {
  test("validates generated query/path contracts and keeps board route encoded", async () => {
    const transport: SignalsOntologyReadTransport = {
      get: vi.fn(async (path: string): Promise<HttpTransportResponse> => {
        if (path.includes("signals/review")) return response({ data: [signal], meta: { include_all: false, limit: 100 } })
        return response(signal)
      }),
    }
    const api = createSignalsOntologyReadApi(runtime, {
      board: "team/one #",
      transport,
    })

    const rows = await api.reviewSignals({ statuses: ["open"], kinds: ["agent_cli_friction"], task: "team/one#1", includeAll: false, limit: 100 })

    expect(rows).toHaveLength(1)
    expect(transport.get).toHaveBeenCalledWith(
      "/api/v1/boards/team%2Fone%20%23/signals/review?status=open&kind=agent_cli_friction&task_ref=team%2Fone%231&include_all=false&limit=100",
      undefined,
    )
  })

  test("rejects malformed response payloads at the generated validator boundary", async () => {
    const transport: SignalsOntologyReadTransport = {
      get: vi.fn(async (): Promise<HttpTransportResponse> => response({ data: [{ id: "sig_1" }] })),
    }
    const api = createSignalsOntologyReadApi(runtime, { board: "default", transport })

    await expect(api.reviewSignals()).rejects.toMatchObject({ name: "ContractValidationError", contractId: "api.review-signals.response" })
  })
})
