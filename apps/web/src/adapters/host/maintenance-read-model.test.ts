import { describe,expect,test,vi } from 'vitest'
import type { RpcTransport } from '../../application/data/rpc-transport'
import { RpcTransportError } from '../../application/data/rpc-transport'
import { createMaintenanceApi } from './maintenance-api'

const fixtures = import.meta.glob('../../lib/api/generated/fixtures/*-response.valid.json', { eager: true, import: 'default' })
function response(method: string) {
  const key = method.replace(/([a-z0-9])([A-Z])/g, '$1-$2').toLowerCase()
  const payload = fixtures[`../../lib/api/generated/fixtures/api-${key}-response.valid.json`]
  if (!payload) throw new Error(`缺少契约 fixture ${method}`)
  return { payload: structuredClone(payload), bytes: 1 }
}
function setup() {
  const call = vi.fn<RpcTransport['call']>(async ({ method }) => response(method))
  return { call, api: createMaintenanceApi({ transport: { call } }) }
}

describe('管理命名 RPC', () => {
  test('status、doctor、stats、search 保留完整响应和板选择', async () => {
    const { api, call } = setup()
    await expect(api.status()).resolves.toEqual((response('MaintenanceStatus').payload as { data: unknown }).data)
    await expect(api.doctor()).resolves.toEqual((response('Doctor').payload as { data: unknown }).data)
    await expect(api.stats('board-two')).resolves.toEqual((response('GetStats').payload as { data: unknown }).data)
    await expect(api.searchStatus('board-two')).resolves.toEqual((response('SearchStatus').payload as { data: unknown }).data)
    expect(call.mock.calls.map(([r]) => r.method)).toEqual(['MaintenanceStatus', 'Doctor', 'GetStats', 'SearchStatus'])
    expect(call.mock.calls[2]?.[0]).toEqual({ method: 'GetStats', query: { board: 'board-two' } })
    expect(call.mock.calls[3]?.[0]).toEqual({ method: 'SearchStatus', query: { board: 'board-two' } })
  })
  test('备份、导入导出和维护使用各自具名调用，保留路径/replace/owner', async () => {
    const { api, call } = setup()
    await api.backup('/requested/backup.sqlite')
    await api.exportData('/requested/export.jsonl')
    await api.importData('/requested/export.jsonl', true)
    await api.vacuum()
    await api.checkpoint()
    await api.maintenanceRun(' ')
    await api.maintenanceRebuild(' actor ')
    await api.maintenanceCleanup('actor')
    expect(call.mock.calls.map(([r]) => r.method)).toEqual(['MaintenanceBackup', 'MaintenanceExport', 'MaintenanceImport', 'MaintenanceVacuum', 'Checkpoint', 'MaintenanceRun', 'MaintenanceRebuild', 'MaintenanceCleanup'])
    expect(call.mock.calls[0]?.[0].input).toEqual({ path: '/requested/backup.sqlite' })
    expect(call.mock.calls[2]?.[0].input).toEqual({ path: '/requested/export.jsonl', replace: true })
    expect(call.mock.calls[5]?.[0].input).toEqual({ owner: null, action: 'run' })
    expect(call.mock.calls[6]?.[0].input).toEqual({ owner: 'actor', action: null })
  })
  test('契约异常不变成成功结果', async () => {
    const { api, call } = setup()
    call.mockResolvedValueOnce({ payload: { data: { owner: {} } }, bytes: 1 })
    await expect(api.status()).rejects.toMatchObject({ kind: 'invalid_contract', contractId: 'api.maintenance-status.response' })
  })
  test('稳定业务代码传到管理页，未执行额外请求', async () => {
    const { api, call } = setup()
    call.mockRejectedValueOnce(new RpcTransportError('http', 'conflict', { status: 409, apiError: { code: 'conflict', message: 'conflict' } }))
    await expect(api.vacuum()).rejects.toMatchObject({ kind: 'http', status: 409, code: 'conflict' })
    expect(call).toHaveBeenCalledTimes(1)
  })
  test('AbortSignal 贯穿调用，取消保持 AbortError', async () => {
    const { api, call } = setup()
    const signal = new AbortController().signal
    const aborted = new DOMException('cancelled', 'AbortError')
    call.mockRejectedValueOnce(aborted)
    await expect(api.doctor(signal)).rejects.toBe(aborted)
    expect(call).toHaveBeenCalledWith({ method: 'Doctor', signal })
  })
  test('缺少 runtime 和注入 channel 时明确失败', () => {
    expect(() => createMaintenanceApi()).toThrow('缺少 runtime transport')
  })
})
