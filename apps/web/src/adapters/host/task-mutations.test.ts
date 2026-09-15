import { describe,expect,test,vi } from 'vitest'
import type { RpcTransport } from '../../application/data/rpc-transport'
import { RpcTransportError } from '../../application/data/rpc-transport'
import type { ClaimTaskIntent,CompleteTaskIntent,StepMutationIntent } from '../../application/data/task-mutations'
import { assertCanonicalBoardSlug } from '../../domain/board-slug'
import type { WebRuntimeConfig } from '../../lib/runtime'
import { createTaskMutationClient } from './task-mutations'

const runtime = {
  apiBaseUrl: '/__kb_api__', webBasePath: '/app/', actor: 'web-user', defaultBoard: 'default',
  serverVersion: '3.1.0', protocolVersion: 'v1', webBuildId: 'sha256:test',
} satisfies WebRuntimeConfig
const fixtures = import.meta.glob('../../lib/api/generated/fixtures/*-response.valid.json', { eager: true, import: 'default' })
function fixture(method: string) {
  const key = method.replace(/([a-z0-9])([A-Z])/g, '$1-$2').toLowerCase()
  const payload = fixtures[`../../lib/api/generated/fixtures/api-${key}-response.valid.json`]
  if (!payload) throw new Error(`缺少真实契约 fixture: ${method}`)
  return structuredClone(payload)
}
function setup(actor?: string, board = 'default') {
  const call = vi.fn<RpcTransport['call']>(async ({ method }) => ({ payload: fixture(method), bytes: 1 }))
  const client = createTaskMutationClient(runtime, assertCanonicalBoardSlug(board), { transport: { call }, actor })
  return { call, client }
}

describe('命名任务 RPC', () => {
  test('创建保持 canonical board、幂等键和中文 actor', async () => {
    const { client, call } = setup(' 作者甲 ', 'team-two')
    await client.createTask({ title: '任务', description: '说明', idempotency_key: 'same-key' })
    expect(call).toHaveBeenCalledWith({ method: 'CreateTask', path: { board: 'team-two' }, input: { title: '任务', description: '说明', idempotency_key: 'same-key', actor: '作者甲' }, actor: '作者甲' })
  })
  test('无效 actor preference 采用 runtime actor', async () => {
    const { client, call } = setup('bad\nactor')
    await client.createTask({ title: '任务' })
    expect(call).toHaveBeenCalledWith(expect.objectContaining({ actor: 'web-user', input: { title: '任务', actor: 'web-user' } }))
  })
  test('更新保持 CAS 和 null 清空，不填入未修改字段', async () => {
    const { client, call } = setup()
    await client.updateTask('t_1', { expected_lock_version: 7, description: null })
    expect(call).toHaveBeenCalledWith({ method: 'UpdateTask', path: { task_id: 't_1' }, input: { expected_lock_version: 7, description: null, actor: 'web-user' }, actor: 'web-user' })
    expect(call.mock.calls[0]?.[0].input).not.toHaveProperty('title')
  })
  test('标签 ID 按字段保留，添加/删除使用各自命名操作', async () => {
    const { client, call } = setup()
    await client.addTaskLabel('t_1', { name: 'urgent', create_missing: true })
    await client.removeTaskLabel('t_1', 'l_%中')
    expect(call.mock.calls.map(([r]) => r.method)).toEqual(['AddTaskLabel', 'RemoveTaskLabel'])
    expect(call.mock.calls[1]?.[0]).toEqual({ method: 'RemoveTaskLabel', path: { task_id: 't_1', label_id: 'l_%中' }, actor: 'web-user' })
  })
  test('所有可见状态动作保留独立请求和响应', async () => {
    const { client, call } = setup()
    const claim: ClaimTaskIntent = { worker_profile: 'default' }
    const complete: CompleteTaskIntent = { claim_token: 'claim-token', summary: 'done' }
    await client.transitionTask('t_1', 'specify', { description: 'ready' })
    await client.transitionTask('t_1', 'promote')
    await client.transitionTask('t_1', 'claim', claim)
    await client.transitionTask('t_1', 'heartbeat', { claim_token: 'claim-token' })
    await client.transitionTask('t_1', 'complete', complete)
    await client.transitionTask('t_1', 'submit-review', { summary: 'review' })
    await client.transitionTask('t_1', 'block', { reason: 'waiting' })
    await client.transitionTask('t_1', 'unblock')
    await client.transitionTask('t_1', 'archive')
    expect(call.mock.calls.map(([r]) => r.method)).toEqual(['SpecifyTask', 'PromoteTask', 'ClaimTask', 'HeartbeatTask', 'CompleteTask', 'SubmitReviewTask', 'BlockTask', 'UnblockTask', 'ArchiveTask'])
    expect(call.mock.calls[2]?.[0].input).toMatchObject({ worker_profile: 'default', actor: 'web-user' })
    expect(call.mock.calls[3]?.[0].input).toMatchObject({ claim_token: 'claim-token', actor: 'web-user' })
    expect(call.mock.calls[4]?.[0].input).toMatchObject(complete)
    const invalidCall = () => {
      // @ts-expect-error heartbeat 在公开接口要求 claim token。
      return client.transitionTask('t_1', 'heartbeat', {})
    }
    expect(invalidCall).toBeTypeOf('function')
  })
  test('依赖、步骤、评论与执行计划保持各自完整契约', async () => {
    const { client, call } = setup()
    await client.listDependencies('t_1')
    await client.addDependency('t_1', 't_parent')
    await client.removeDependency('t_1', 't_parent')
    await client.listSteps('t_1')
    await client.createStep('t_1', { title: '验证' })
    await client.markExecutionPlanNotRequired('t_1', { reason: '无需步骤' })
    await client.listComments('t_1')
    await client.createComment('t_1', { body: '备注' })
    expect(call.mock.calls.map(([r]) => r.method)).toEqual(['ListDependencies', 'AddDependency', 'RemoveDependency', 'ListSteps', 'CreateStep', 'MarkExecutionPlanNotRequired', 'ListComments', 'CreateComment'])
    expect(call.mock.calls[2]?.[0].path).toEqual({ child_task_id: 't_1', parent_task_id: 't_parent' })
    expect(call.mock.calls[7]?.[0].input).toEqual({ body: '备注', author: 'web-user' })
    expect(call.mock.calls[0]?.[0]).not.toHaveProperty('actor')
  })
  test.each([
    { action: 'update', input: { title: '新标题', unlink_task: false } }, { action: 'remove' },
    { action: 'complete', input: { note: '完成依据' } }, { action: 'skip', input: { reason: '跳过原因' } }, { action: 'reopen', input: { reason: '重新验证' } },
  ] satisfies StepMutationIntent[])('步骤动作 $action 只提交命名操作', async command => {
    const { client, call } = setup()
    await client.mutateStep('t_1', 's_1', command)
    expect(call).toHaveBeenCalledWith(expect.objectContaining({ path: { task_id: 't_1', step_id: 's_1' }, actor: 'web-user' }))
    expect(call.mock.calls[0]?.[0].method).toBe(`${command.action[0]?.toUpperCase()}${command.action.slice(1)}Step`)
  })
  test('附件内容保持原始 bytes，元数据列表和删除为命名操作', async () => {
    const { client, call } = setup()
    const content = new Uint8Array([0, 255, 13])
    await client.listAttachments('t_1')
    await client.createAttachment('t_1', { filename: '内容.bin', content, content_type: 'application/octet-stream' })
    await client.deleteAttachment('t_1', 'a_%中')
    expect(call.mock.calls.map(([r]) => r.method)).toEqual(['ListAttachments', 'CreateAttachment', 'DeleteAttachment'])
    expect(call.mock.calls[1]?.[0].input).toMatchObject({ filename: '内容.bin', content_type: 'application/octet-stream', content })
    expect((call.mock.calls[1]?.[0].input as { content: Uint8Array }).content).toBe(content)
    expect(call.mock.calls[2]?.[0].path).toEqual({ task_id: 't_1', attachment_id: 'a_%中' })
  })
  test('保留既有 number[] 调用，但拒绝不合法字节', async () => {
    const { client, call } = setup()
    await client.createAttachment('t_1', { filename: 'x.bin', content: [0, 255] })
    expect(call.mock.calls[0]?.[0].input).toMatchObject({ content: new Uint8Array([0, 255]) })
    expect(() => client.createAttachment('t_1', { filename: 'x.bin', content: [256] })).toThrow()
    expect(call).toHaveBeenCalledTimes(1)
  })
  test('标签建议 query、取消和降级信息保留', async () => {
    const { client, call } = setup()
    const signal = new AbortController().signal
    const query = { limit: 3, candidate_limit: 32, atom_limit: 80, max_selected_labels: 4, min_score: 0.15 }
    const result = await client.suggestTaskLabels('t_1', query, { signal })
    expect(call).toHaveBeenCalledWith({ method: 'SuggestTaskLabels', path: { task_id: 't_1' }, query, signal })
    expect(result).toEqual(fixture('SuggestTaskLabels'))
  })
  test('契约异常不会暴露为成功的 mutation result', async () => {
    const { client, call } = setup()
    call.mockResolvedValueOnce({ payload: { data: { id: 'missing-fields' } }, bytes: 1 })
    await expect(client.createTask({ title: '任务' })).rejects.toMatchObject({ name: 'ContractValidationError', contractId: 'api.create-task.response' })
  })
  test('冲突和取消原样交给 controller，重试保留幂等键', async () => {
    const { client, call } = setup()
    const error = new RpcTransportError('http', 'conflict', { status: 409, apiError: { code: 'claim_conflict', message: 'conflict' } })
    call.mockRejectedValueOnce(error)
    const intent = { title: '任务', idempotency_key: 'retry-key' }
    await expect(client.createTask(intent)).rejects.toBe(error)
    await client.createTask(intent)
    expect(call.mock.calls[0]?.[0].input).toEqual(call.mock.calls[1]?.[0].input)
    const aborted = new DOMException('cancelled', 'AbortError')
    call.mockRejectedValueOnce(aborted)
    await expect(client.updateTask('t_1', { expected_lock_version: 1 })).rejects.toBe(aborted)
    expect(call).toHaveBeenCalledTimes(3)
  })
})
