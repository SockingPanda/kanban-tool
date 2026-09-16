import { execFile } from 'node:child_process'
import { createHash, randomUUID } from 'node:crypto'
import { createReadStream } from 'node:fs'
import { writeFile } from 'node:fs/promises'
import { promisify } from 'node:util'
import { fromBinary } from '@bufbuild/protobuf'
import { expect, test } from '@playwright/test'
import { I64_MAX, I64_MIN, U64_MAX } from '../src/domain/integer'
import { DtoApiCreateTaskStatus } from '../src/generated/rpc/kanban/v1/dto_pb'
import { UpdateTaskRequestSchema } from '../src/generated/rpc/kanban/v1/kanban_pb'
import { parseJson, stringifyJson } from '../src/lib/lossless-json'
import { createRpcClients } from '../src/lib/rpc/client'
import { decodeJson, encodeJson } from '../src/lib/rpc/value-codec'

const execute = promisify(execFile)
const baseUrl = process.env.KANBAN_RELEASE_BASE_URL
const cliBinary = process.env.KANBAN_RELEASE_CLI

async function fileSha256(path: string): Promise<string> {
  const hash = createHash('sha256')
  for await (const chunk of createReadStream(path)) hash.update(chunk)
  return hash.digest('hex')
}

test('实际 Host：完整 64 位日期和动态 JSON 经页面、原生 CLI 与二进制 mutation 保真', async ({ page, request }, info) => {
  if (!baseUrl || !cliBinary) throw new Error('64 位真实 Host 验收需要 KANBAN_RELEASE_BASE_URL 和 KANBAN_RELEASE_CLI')
  const rpc = createRpcClients(baseUrl)
  const board = `integers-${randomUUID().slice(0, 8)}`
  const metadata = {
    signed_min: I64_MIN, signed_max: I64_MAX, unsigned_max: U64_MAX,
    signed_min_text: String(I64_MIN), signed_max_text: String(I64_MAX), unsigned_max_text: String(U64_MAX),
  }
  await rpc.business.createBoard({ slug: board, name: '完整整数真实 Host 验收' })
  const created = await rpc.business.createTask({
    board, title: '64 位初始任务', status: DtoApiCreateTaskStatus.TODO,
    scheduledAt: I64_MIN, dueAt: I64_MAX, idempotencyKey: randomUUID(),
  })
  if (!created.data?.id) throw new Error('Host 未返回 task id')
  const taskId = created.data.id
  expect(created.data.scheduledAt).toBe(I64_MIN)
  expect(created.data.dueAt).toBe(I64_MAX)
  const comment = await rpc.business.createComment({
    taskId, body: '64 位元数据', author: '正式 RPC 作者', metadata: encodeJson(metadata), idempotencyKey: randomUUID(),
  })
  expect(Object.fromEntries(Object.entries(comment.data?.metadata?.value ?? {}).map(([key, value]) => [key, decodeJson(value)]))).toEqual(metadata)

  const runtimeResponse = await request.get(`${baseUrl}/app/runtime.json`)
  expect(runtimeResponse.ok()).toBe(true)
  const runtime = await runtimeResponse.json() as { webBuildId: string; protocolVersion: string }
  const manifestResponse = await request.get(`${baseUrl}/app/manifest.json`)
  expect(manifestResponse.ok()).toBe(true)
  expect((await manifestResponse.json()).buildId).toBe(runtime.webBuildId)

  const errors: string[] = []
  const updates: Buffer[] = []
  const requests: { path: string; method: string; contentType?: string }[] = []
  page.on('pageerror', error => errors.push(error.message))
  page.on('request', outgoing => {
    const path = new URL(outgoing.url()).pathname
    requests.push({ path, method: outgoing.method(), contentType: outgoing.headers()['content-type'] })
    if (path === '/kanban.v1.KanbanService/UpdateTask') {
      const frame = outgoing.postDataBuffer()
      if (frame) updates.push(frame)
    }
  })
  await page.goto(`/app/boards/${board}/list?task=${taskId}`)
  await expect(page.getByTestId('task-inspector')).toBeVisible()
  await expect(page.locator('[data-runtime-web-build-id]')).toHaveAttribute('data-runtime-web-build-id', runtime.webBuildId)
  const title = page.getByRole('textbox', { name: '任务标题', exact: true })
  await expect(title).toHaveValue('64 位初始任务')
  await page.getByText('运行记录与任务操作', { exact: true }).click()
  await page.getByRole('button', { name: '编辑更多属性', exact: true }).click()
  const form = page.getByTestId('inspector-edit-form')
  await expect(form.locator('[name="task-scheduled-at"]')).toHaveValue(String(I64_MIN))
  await expect(form.locator('[name="task-due-at"]')).toHaveValue(String(I64_MAX))
  await form.locator('[name="task-title"]').fill('页面精确保存后的标题')
  await form.getByRole('button', { name: '保存', exact: true }).click()
  await expect(form).toBeHidden()
  await expect(title).toHaveValue('页面精确保存后的标题')

  expect(updates).toHaveLength(1)
  const frame = updates[0]!
  expect(frame[0]).toBe(0)
  expect(frame.readUInt32BE(1)).toBe(frame.length - 5)
  const update = fromBinary(UpdateTaskRequestSchema, frame.subarray(5))
  expect(update.taskId).toBe(taskId)
  expect(update.title).toBe('页面精确保存后的标题')
  expect(update.expectedLockVersion).toBe(created.data.lockVersion)
  expect(update.scheduledAt?.change).toEqual({ case: 'value', value: I64_MIN })
  expect(update.dueAt?.change).toEqual({ case: 'value', value: I64_MAX })
  const saved = await rpc.business.getTask({ taskId })
  expect(saved.data).toMatchObject({ scheduledAt: I64_MIN, dueAt: I64_MAX, lockVersion: created.data.lockVersion! + 1n })

  await page.reload()
  await expect(title).toHaveValue('页面精确保存后的标题')
  await page.getByText('运行记录与任务操作', { exact: true }).click()
  await page.getByRole('button', { name: '编辑更多属性', exact: true }).click()
  await expect(form.locator('[name="task-scheduled-at"]')).toHaveValue(String(I64_MIN))
  await expect(form.locator('[name="task-due-at"]')).toHaveValue(String(I64_MAX))
  await form.getByRole('button', { name: '取消', exact: true }).click()
  await page.getByRole('button', { name: /^讨论/ }).click()
  const discussion = page.getByTestId('task-discussion')
  await expect(discussion).toContainText('64 位元数据')
  await discussion.getByText('附加信息', { exact: true }).click()
  const metadataText = discussion.locator('pre').filter({ hasText: '"unsigned_max"' })
  for (const [key, value] of Object.entries(metadata)) {
    await expect(metadataText).toContainText(`"${key}": ${typeof value === 'string' ? JSON.stringify(value) : String(value)}`)
  }
  const draft = page.getByRole('textbox', { name: '评论内容', exact: true })
  await draft.fill('外部更新期间保留的评论草稿')
  await rpc.business.updateTask({ taskId, title: '外部正式 RPC 更新', expectedLockVersion: saved.data!.lockVersion })
  await rpc.business.createComment({ taskId, body: '外部正式 RPC 评论', author: '外部作者', idempotencyKey: randomUUID() })
  await expect(title).toHaveValue('外部正式 RPC 更新')
  await expect(discussion).toContainText('外部正式 RPC 评论')
  await expect(draft).toHaveValue('外部更新期间保留的评论草稿')
  await expect(draft).toBeFocused()
  await expect(metadataText).toContainText('"unsigned_max": 18446744073709551615')
  const final = await rpc.business.getTask({ taskId })
  expect(final.data).toMatchObject({ scheduledAt: I64_MIN, dueAt: I64_MAX })

  // 原生 CLI 输出先检查原始数字 token，不能用 JSON.parse 把错误的舍入再藏起来。
  const cli = async (...args: string[]) => (await execute(cliBinary, [
    '--json', '--server-url', baseUrl, '--board', board, ...args,
  ], { timeout: 10_000 })).stdout
  const taskJson = await cli('task', 'show', taskId)
  const commentsJson = await cli('comment', 'list', taskId)
  expect(taskJson).toMatch(/"scheduled_at"\s*:\s*-9223372036854775808\s*[,}]/)
  expect(taskJson).toMatch(/"due_at"\s*:\s*9223372036854775807\s*[,}]/)
  for (const [key, value] of Object.entries(metadata)) {
    const token = typeof value === 'string' ? JSON.stringify(value) : String(value)
    expect(commentsJson).toMatch(new RegExp(`"${key}"\\s*:\\s*${token}\\s*[,}]`))
  }
  expect(parseJson(taskJson)).toMatchObject({ data: { scheduled_at: I64_MIN, due_at: I64_MAX, title: '外部正式 RPC 更新' } })
  expect(parseJson(commentsJson)).toMatchObject({ data: expect.arrayContaining([expect.objectContaining({ metadata })]) })
  expect(errors).toEqual([])
  expect(requests.filter(entry => entry.path.startsWith('/api/v1/'))).toEqual([])
  const businessRequests = requests.filter(entry => entry.path.startsWith('/kanban.v1.'))
  expect(businessRequests.length).toBeGreaterThan(0)
  expect(businessRequests.every(entry => entry.method === 'POST' && entry.contentType === 'application/grpc-web+proto')).toBe(true)

  const artifacts: [string, string | Buffer, string][] = [
    ['update-task.grpc-web.bin', frame, 'application/octet-stream'],
    ['task.cli.json', taskJson, 'application/json'],
    ['comments.cli.json', commentsJson, 'application/json'],
    ['integer-host-proof.json', stringifyJson({
      baseUrl, board, taskId, runtime, browser: info.project.name,
      hostLabel: process.env.KANBAN_RELEASE_HOST_LABEL ?? '调用方提供的候选 Host',
      hostPid: process.env.KANBAN_RELEASE_HOST_PID ?? null,
      cliBinary, cliSha256: await fileSha256(cliBinary),
      expectedLockVersion: update.expectedLockVersion, scheduledAt: final.data?.scheduledAt, dueAt: final.data?.dueAt,
      metadata, requests, pageErrors: errors, refreshedValuesRetained: true, externalUpdateRetainedDraftAndFocus: true,
    }, 2) + '\n', 'application/json'],
  ]
  for (const [name, contents, contentType] of artifacts) {
    const path = info.outputPath(name)
    await writeFile(path, contents)
    await info.attach(name, { path, contentType })
  }
  await page.screenshot({ path: info.outputPath('integer-real-host.png'), fullPage: true })
})
