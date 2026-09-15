import { describe, expect, test } from "vitest"

import contracts from "../../lib/api/generated/contracts.json"
import {
  ContractValidationError,
  isGeneratedContractId,
  parseApiListTasksQuery,
  parseApiErrorResponse,
  validateContract,
  validators,
} from "../../lib/api/generated/test-only"
import { parseApiBoardTaskMapResponse } from "../../lib/api/generated/contracts/api-board-task-map-response"
import { parseApiHealthResponse } from "../../lib/api/generated/contracts/api-health-response"
import {
  getOperation,
  operationById,
} from "../../lib/api/generated/operations"
import { apiEventDataValidator, parseApiEventData } from "../../lib/api/generated/contracts/api-event-data"

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
const fixtures = import.meta.glob("../../lib/api/generated/fixtures/*.json", {
  eager: true,
  import: "default",
}) as Record<string, unknown>

function fixture(path: string): unknown {
  const value = fixtures[`../../lib/api/generated/${path}`]
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

    expect(checked).toBe(contractRecords.length)
    expect(checked).toBeGreaterThan(0)
    expect(contractRecords.map(contract => contract.id)).toContain("api.event.data")
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

  test("parses generated health and board responses with valid and invalid fixtures", () => {
    const validHealth = fixture("fixtures/api-health-response.valid.json")
    const invalidHealth = fixture("fixtures/api-health-response.invalid.json")
    const validBoard = fixture("fixtures/api-board-task-map-response.valid.json")
    const invalidBoard = fixture("fixtures/api-board-task-map-response.invalid.json")

    expect(parseApiHealthResponse(validHealth)).toBe(validHealth)
    expect(() => parseApiHealthResponse(invalidHealth)).toThrow(ContractValidationError)
    expect(parseApiBoardTaskMapResponse(validBoard)).toBe(validBoard)
    expect(() => parseApiBoardTaskMapResponse(invalidBoard)).toThrow(ContractValidationError)
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

  test("operation lookup 描述正式 RPC 与保留的 DTO 校验入口", () => {
    const task = getOperation('api.get-task')
    expect(operationById['api.get-task']).toBe(task)
    expect(task).toMatchObject({ service: 'kanban.v1.KanbanService', method: 'GetTask', path: '/kanban.v1.KanbanService/GetTask', serverStreaming: false })
    expect(task.obligations.path).toEqual({ kind: 'contract', contractId: 'api.get-task.path' })
    expect(getOperation('api.list-events').obligations.success).toEqual({ kind: 'contract', contractId: 'api.list-events.response' })
    expect(Object.values(operationById).every(operation => operation.path.startsWith('/kanban.v1.'))).toBe(true)
  })

  test("审计数据校验保留已知 payload 与未知事件，不绑定传输帧", () => {
    const valid = recordFixture('fixtures/api-event-data.valid.json')
    expect(parseApiEventData(valid)).toBe(valid)
    expect(apiEventDataValidator({ ...valid, kind: 'task.attachment.created' })).toBe(true)
    expect(apiEventDataValidator({ ...valid, payload: null })).toBe(false)
    expect(apiEventDataValidator({ event_id: 'missing-kind' })).toBe(false)
    expect(apiEventDataValidator(null)).toBe(false)
    expect(() => parseApiEventData({ ...valid, payload: null })).toThrow(ContractValidationError)
  })
})
