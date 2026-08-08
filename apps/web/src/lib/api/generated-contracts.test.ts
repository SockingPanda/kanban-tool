import { describe, expect, test } from "vitest"

import contracts from "./generated/contracts.json"
import {
  ContractValidationError,
  isGeneratedContractId,
  parseApiListTasksQuery,
  parseApiErrorResponse,
  validateContract,
  validators,
} from "./generated/test-only"
import {
  getOperation,
  operationById,
} from "./generated/operations"
import {
  knownSseEventKinds,
  parseSseEvent,
} from "./generated/sse"

type ContractRecord = {
  id: string
  validFixture: string
  invalidFixture: string
}

const contractRecords: readonly ContractRecord[] = contracts.map((contract) => ({
  id: contract.id,
  validFixture: contract.validFixture,
  invalidFixture: contract.invalidFixture,
}))
const fixtures = import.meta.glob("./generated/fixtures/*.json", {
  eager: true,
  import: "default",
}) as Record<string, unknown>

function fixture(path: string): unknown {
  const value = fixtures[`./generated/${path}`]
  expect(value, `missing generated fixture ${path}`).toBeDefined()
  return value
}

function recordFixture(path: string): Record<string, unknown> {
  const value = fixture(path)
  if (!isRecord(value)) throw new Error(`generated fixture ${path} is not an object`)
  return value
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

describe("generated Web contracts", () => {
  test("accept every generated valid fixture and reject every invalid fixture", () => {
    let checked = 0

    for (const contract of contractRecords) {
      expect(isGeneratedContractId(contract.id), `unknown generated contract ${contract.id}`).toBe(true)
      if (!isGeneratedContractId(contract.id)) continue
      const validator = validators[contract.id]
      expect(validator, contract.id).toEqual(expect.any(Function))
      expect(validator(fixture(contract.validFixture)), `${contract.id} valid`).toBe(true)
      expect(validator(fixture(contract.invalidFixture)), `${contract.id} invalid`).toBe(false)
      checked += 1
    }

    expect(checked).toBe(200)
  })

  test("keeps unknown contract ids out of the validation boundary", () => {
    expect(isGeneratedContractId("api.not-generated")).toBe(false)
    expect(() => {
      // @ts-expect-error 故意模拟未生成 contract id 的动态边界。
      validateContract("api.not-generated", {})
    }).toThrow(/Unknown generated contract id/)
  })

  test("validates and parses the shared API error contract", () => {
    const valid = fixture("fixtures/api-error-response.valid.json")
    const invalid = fixture("fixtures/api-error-response.invalid.json")

    expect(validateContract("api.error.response", valid)).toBe(true)
    expect(parseApiErrorResponse(valid)).toBe(valid)
    expect(validateContract("api.error.response", invalid)).toBe(false)
    try {
      parseApiErrorResponse(invalid)
      throw new Error("invalid API error fixture unexpectedly parsed")
    } catch (error) {
      if (!(error instanceof ContractValidationError)) throw error
      expect(error.errors?.length).toBeGreaterThan(0)
    }
  })

  test("rejects unsafe JSON numbers before AJV while accepting safe bounds", () => {
    const safe = { offset: Number.MAX_SAFE_INTEGER }
    const unsafe = { offset: Number.MAX_SAFE_INTEGER + 1 }

    expect(validateContract("api.list-tasks.query", safe)).toBe(true)
    expect(validateContract("api.list-tasks.query", unsafe)).toBe(false)
    try {
      parseApiListTasksQuery(unsafe)
      throw new Error("unsafe JSON number unexpectedly parsed")
    } catch (error) {
      if (!(error instanceof ContractValidationError)) throw error
      expect(error.errors).toEqual(
        expect.arrayContaining([
          expect.objectContaining({
            instancePath: "/offset",
            keyword: "safeNumber",
          }),
        ]),
      )
    }
  })

  test("provides typed O(1) operation lookup for HTTP and SSE", () => {
    const task = getOperation("api.get-task")
    expect(operationById["api.get-task"]).toBe(task)
    expect(task.method).toBe("GET")
    expect(task.path).toBe("/api/v1/tasks/:task_id")
    expect(task.obligations.path).toEqual({
      kind: "contract",
      contractId: "api.get-task.path",
    })

    const stream = getOperation("sse.stream-events")
    expect(stream.method).toBe("GET")
    expect(stream.path).toBe("/api/v1/stream/events")
    expect(stream.obligations.query).toEqual({
      kind: "contract",
      contractId: "sse.stream-events.query",
    })
    expect(stream.obligations.sse).toEqual({
      kind: "contract",
      contractId: "sse.event.data",
    })
  })

  test("classifies known, real unknown, known-invalid, and invalid SSE envelopes", () => {
    const valid = recordFixture("fixtures/sse-event-data.valid.json")
    const known = parseSseEvent(valid)
    expect(known).toBe(valid)
    expect(knownSseEventKinds).toContain("task.created")
    expect(known).not.toHaveProperty("reason")

    const unknown = { ...valid, kind: "task.attachment.created" }
    const unknownResult = parseSseEvent(unknown)
    expect(unknownResult).toEqual({
      kind: "task.attachment.created",
      raw: unknown,
      envelope: unknown,
      reason: "unknown_kind",
    })
    expect(knownSseEventKinds).not.toContain("task.attachment.created")

    const knownInvalid = { ...valid, payload: null }
    expect(parseSseEvent(knownInvalid)).toEqual({
      kind: "task.created",
      raw: knownInvalid,
      envelope: knownInvalid,
      reason: "known_payload_invalid",
    })

    const invalidEnvelope = { event_id: "missing-kind" }
    expect(parseSseEvent(invalidEnvelope)).toEqual({
      kind: null,
      raw: invalidEnvelope,
      envelope: invalidEnvelope,
      reason: "invalid_envelope",
    })
    expect(parseSseEvent(null)).toEqual({
      kind: null,
      raw: null,
      envelope: null,
      reason: "invalid_envelope",
    })
  })
})
