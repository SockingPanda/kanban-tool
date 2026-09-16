import { describe, expect, expectTypeOf, test } from 'vitest'
import { I64_MIN, I64_MAX, U64_MAX, type Integer } from '../../domain/integer'
import taskFixture from '../../lib/api/generated/fixtures/api-get-task-response.valid.json'
import eventFixture from '../../lib/api/generated/fixtures/api-event-data.valid.json'
import healthFixture from '../../lib/api/generated/fixtures/api-health-response.valid.json'
import { parseApiGetTaskResponse, type ApiGetTaskResponseContract } from '../../lib/api/generated/contracts/api-get-task-response'
import { apiListTasksQueryValidator } from '../../lib/api/generated/contracts/api-list-tasks-query'
import { apiEventDataValidator, type ApiEventDataContract } from '../../lib/api/generated/contracts/api-event-data'
import { apiHealthResponseValidator } from '../../lib/api/generated/contracts/api-health-response'
import { apiUpdateTaskRequestValidator } from '../../lib/api/generated/contracts/api-update-task-request'
import backupFixture from '../../lib/api/generated/fixtures/api-maintenance-backup-response.valid.json'
import { apiMaintenanceBackupResponseValidator } from '../../lib/api/generated/contracts/api-maintenance-backup-response'
import { decodeMaintenanceBackupResponse, encodeMaintenanceBackupResponse } from '../../lib/rpc/codec.generated'

describe('generated 64 位 DTO 与静态校验器', () => {
  test('类型只扩展整数，nullable、优先级和动态 JSON 保持各自语义', () => {
    expectTypeOf<ApiGetTaskResponseContract['data']['lock_version']>().toEqualTypeOf<Integer>()
    expectTypeOf<ApiGetTaskResponseContract['data']['due_at']>().toEqualTypeOf<Integer | null>()
    expectTypeOf<ApiGetTaskResponseContract['data']['priority']>().toEqualTypeOf<number>()
    expectTypeOf<ApiEventDataContract['id']>().toEqualTypeOf<Integer>()
  })

  test('完整任务与嵌套动态 JSON 接受 signed/unsigned 极值但不接受数值字符串或已舍入 number', () => {
    const value = { data: { ...taskFixture.data, seq: I64_MAX, position: I64_MIN, lock_version: I64_MAX, due_at: I64_MIN, metadata: { integer: U64_MAX, text: String(U64_MAX) } } }
    const parsed = parseApiGetTaskResponse(value)
    expect(parsed).toBe(value)
    expect(parsed.data.lock_version).toBe(I64_MAX)
    for (const invalid of [I64_MAX + 1n, I64_MIN - 1n, String(I64_MAX), Number.MAX_SAFE_INTEGER + 1, 0.25]) {
      expect(() => parseApiGetTaskResponse({ data: { ...value.data, lock_version: invalid } })).toThrow()
    }
    expect(() => parseApiGetTaskResponse({ data: { ...value.data, priority: 1n } })).toThrow()
    expect(() => parseApiGetTaskResponse({ data: { ...value.data, metadata: { invalid: U64_MAX + 1n } } })).toThrow()
  })

  test('offset 的精确 schema 上界、CAS 输入与无 format 的事件整数均完整校验', () => {
    expect(apiListTasksQueryValidator({ offset: I64_MAX, limit: 100n })).toBe(true)
    for (const offset of [-1n, I64_MAX + 1n, String(I64_MAX)]) expect(apiListTasksQueryValidator({ offset })).toBe(false)
    expect(apiListTasksQueryValidator({ limit: 1001n })).toBe(false)
    expect(apiUpdateTaskRequestValidator({ expected_lock_version: I64_MAX, scheduled_at: I64_MIN, due_at: null })).toBe(true)
    expect(apiEventDataValidator({ ...eventFixture, id: I64_MAX, created_at: I64_MIN })).toBe(true)
    expect(apiEventDataValidator({ ...eventFixture, id: I64_MAX + 1n })).toBe(false)
    expect(apiHealthResponseValidator(healthFixture)).toBe(true)
  })

  test('具名 uint64 response 经过 codec 和 validator 后保留 u64::MAX', () => {
    const value = { data: { ...backupFixture.data, bytes: U64_MAX } }
    const decoded = decodeMaintenanceBackupResponse(encodeMaintenanceBackupResponse(value))
    expect(apiMaintenanceBackupResponseValidator(decoded)).toBe(true)
    expect(decoded).toEqual(value)
    for (const bytes of [-1n, U64_MAX + 1n, String(U64_MAX)]) expect(apiMaintenanceBackupResponseValidator({ data: { ...backupFixture.data, bytes } })).toBe(false)
  })
})
