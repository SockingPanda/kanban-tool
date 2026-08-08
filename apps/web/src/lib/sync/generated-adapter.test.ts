import { describe, expect, test } from "vitest"

import { createGeneratedStreamContractAdapter } from "./generated-adapter"

const adapter = createGeneratedStreamContractAdapter()

function rawEvent(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    id: 7,
    event_id: "event-7",
    board_id: "board-a",
    task_id: "task-a",
    run_id: null,
    kind: "task.created",
    actor: null,
    payload: { status: "todo" },
    created_at: 1_700_000_000,
    ...overrides,
  }
}

describe("generated StreamContractAdapter", () => {
  test("validates SSE header/data identity and wires generated canonical fingerprint", () => {
    const frame = adapter.parseEnvelope({ eventName: "task.created", id: "7", data: JSON.stringify(rawEvent()) })
    expect(frame.status).toBe("valid")
    if (frame.status !== "valid") return

    const result = adapter.validateBusiness(frame.envelope)
    expect(result.status).toBe("known")
    if (result.status !== "known") return
    expect(result.event.canonicalFingerprint).toContain('"id":7')
    expect(result.event.scope.taskId).toBe("task-a")
  })

  test("keeps future kinds lossless and canonicalizes payload key order", () => {
    const first = rawEvent({ kind: "task.attachment.created", payload: { z: 1, a: 2 } })
    const second = rawEvent({ kind: "task.attachment.created", payload: { a: 2, z: 1 } })
    const firstEnvelope = adapter.parsePollingEnvelope(first)
    const secondEnvelope = adapter.parsePollingEnvelope(second)
    expect(firstEnvelope.status).toBe("valid")
    expect(secondEnvelope.status).toBe("valid")
    if (firstEnvelope.status !== "valid" || secondEnvelope.status !== "valid") return
    const firstResult = adapter.validateBusiness(firstEnvelope.envelope)
    const secondResult = adapter.validateBusiness(secondEnvelope.envelope)
    expect(firstResult.status).toBe("unknown")
    expect(secondResult.status).toBe("unknown")
    if (firstResult.status !== "unknown" || secondResult.status !== "unknown") return
    expect(firstResult.event.canonicalFingerprint).toBe(secondResult.event.canonicalFingerprint)
  })

  test("enforces generated task scope metadata without a Web-owned inventory", () => {
    const missingTask = adapter.parsePollingEnvelope(rawEvent({ task_id: null }))
    expect(missingTask.status).toBe("valid")
    if (missingTask.status !== "valid") return
    expect(adapter.validateBusiness(missingTask.envelope)).toEqual({ status: "invalid", code: "task-scope-missing-task-id" })

    const missingParent = adapter.parsePollingEnvelope(
      rawEvent({ kind: "dependency.added", payload: {} }),
    )
    expect(missingParent.status).toBe("valid")
    if (missingParent.status !== "valid") return
    expect(adapter.validateBusiness(missingParent.envelope)).toEqual({ status: "invalid", code: "known_payload_invalid" })
  })

  test("validates the independent generated heartbeat DTO and rejects business fields", () => {
    expect(adapter.isControlFrame({ eventName: "kb-heartbeat", id: null, data: "{}" })).toBe(true)
    expect(adapter.isControlFrame({ eventName: "task.created", id: "7", data: "{}" })).toBe(false)
    expect(adapter.validateControl({ eventName: "kb-heartbeat", id: null, data: "{}" }).status).toBe("valid")
    expect(adapter.validateControl({ eventName: "kb-heartbeat", id: "1", data: "{}" })).toEqual({ status: "invalid", code: "heartbeat-id-forbidden" })
    expect(adapter.validateControl({ eventName: "kb-heartbeat", id: null, data: '{"id":1}' })).toEqual({ status: "invalid", code: "heartbeat-invalid-data" })
  })
})
