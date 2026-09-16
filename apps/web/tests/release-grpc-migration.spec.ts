import { randomUUID } from 'node:crypto'
import { writeFile } from 'node:fs/promises'
import { fromBinary } from '@bufbuild/protobuf'
import { expect, test, type Page, type TestInfo } from '@playwright/test'
import { CreateTaskRequestSchema } from '../src/generated/rpc/kanban/v1/kanban_pb'
import { DtoApiCreateTaskStatus } from '../src/generated/rpc/kanban/v1/dto_pb'
import { createRpcClients } from '../src/lib/rpc/client'

const baseUrl = process.env.KANBAN_RELEASE_BASE_URL
function clients() {
  if (!baseUrl) throw new Error('真实协议验收需要 KANBAN_RELEASE_BASE_URL')
  return createRpcClients(baseUrl)
}
async function seed() {
  const rpc = clients()
  const board = `grpc-ui-${randomUUID().slice(0, 8)}`
  await rpc.business.createBoard({ slug: board, name: 'gRPC 浏览器验收' })
  const created = await rpc.business.createTask({ board, title: 'needle 初始任务', description: '任务说明', status: DtoApiCreateTaskStatus.TODO, idempotencyKey: randomUUID() })
  if (!created.data?.id) throw new Error('Host 未返回 task id')
  return { rpc, board, taskId: created.data.id }
}
function requests(page: Page) {
  const seen: { url: string; method: string; contentType?: string }[] = []
  page.on('request', request => seen.push({ url: request.url(), method: request.method(), contentType: request.headers()['content-type'] }))
  return seen
}
function assertProtocol(seen: ReturnType<typeof requests>) {
  const rpc = seen.filter(request => new URL(request.url).pathname.startsWith('/kanban.'))
  expect(rpc.length).toBeGreaterThan(0)
  expect(rpc.every(request => request.method === 'POST' && request.contentType === 'application/grpc-web+proto')).toBe(true)
  expect(seen.filter(request => new URL(request.url).pathname.startsWith('/api/v1/'))).toEqual([])
}
function unavailable() {
  const text = Buffer.from('grpc-status: 14\r\ngrpc-message: temporary_unavailable\r\n')
  const frame = Buffer.alloc(text.length + 5)
  frame[0] = 0x80
  frame.writeUInt32BE(text.length, 1)
  text.copy(frame, 5)
  return { status: 200, contentType: 'application/grpc-web+proto', body: frame }
}
async function evidence(info: TestInfo, name: string, value: unknown) {
  const path = info.outputPath(`${name}.json`)
  await writeFile(path, JSON.stringify(value, null, 2) + '\n')
  await info.attach(name, { path, contentType: 'application/json' })
}

test('实际 Host：完整查询订阅保留 URL，外部评论和标签写入可见，所有业务请求为 binary gRPC-Web', async ({ page }, info) => {
  const { rpc, board, taskId } = await seed()
  const seen = requests(page)
  await page.goto(`/app/boards/${board}/list?status=todo&q=needle&task=${taskId}`)
  await expect(page.getByTestId('task-list')).toBeVisible()
  await expect(page.getByTestId('list-search')).toHaveValue('needle')
  const title = page.getByRole('textbox', { name: '任务标题', exact: true })
  await expect(title).toHaveValue('needle 初始任务')
  await expect.poll(() => seen.some(request => new URL(request.url).pathname === '/kanban.v1.QueryService/WatchQueries')).toBe(true)
  const current = await rpc.business.getTask({ taskId })
  await rpc.business.updateTask({ taskId, title: 'needle 外部更新', expectedLockVersion: current.data!.lockVersion })
  await expect(title).toHaveValue('needle 外部更新')
  const start = Date.now()
  await rpc.business.createComment({ taskId, body: '外部非卡片评论', author: '外部作者', idempotencyKey: randomUUID() })
  await page.getByRole('button', { name: /^讨论/ }).click()
  await expect(page.getByTestId('task-discussion')).toContainText('外部非卡片评论')
  const commentVisibleMs = Date.now() - start
  await page.getByRole('button', { name: '详情', exact: true }).click()
  await page.locator('summary').filter({ hasText: /^标签$/ }).click()
  await rpc.business.addTaskLabel({ taskId, name: '外部标签', createMissing: true })
  await expect(page.getByTestId('inspector-labels')).toContainText('外部标签')
  await expect(page).toHaveURL(new RegExp(`status=todo&q=needle&task=${taskId}`))
  await page.reload()
  await expect(page.getByTestId('list-search')).toHaveValue('needle')
  await expect(title).toHaveValue('needle 外部更新')
  assertProtocol(seen)
  await evidence(info, 'protocol-requests', { commentVisibleMs, requests: seen })
  await page.screenshot({ path: info.outputPath('grpc-live-list.png'), fullPage: true })
})

test('实际 Host：失败后保留创建草稿和幂等键，重试仅调用原 RPC', async ({ page }, info) => {
  const { board } = await seed()
  const seen = requests(page)
  const attempts: ReturnType<typeof fromBinary<typeof CreateTaskRequestSchema>>[] = []
  await page.route('**/kanban.v1.KanbanService/CreateTask', async route => {
    const bytes = route.request().postDataBuffer()
    if (!bytes || bytes[0] !== 0) throw new Error('创建请求不是 binary gRPC-Web')
    attempts.push(fromBinary(CreateTaskRequestSchema, bytes.subarray(5)))
    if (attempts.length === 1) await route.fulfill(unavailable())
    else await route.fallback()
  })
  await page.goto(`/app/boards/${board}/list`)
  await page.getByTestId('task-create').click()
  await page.getByTestId('task-title-input').fill('保留的 RPC 草稿')
  const dialog = page.getByTestId('task-mutation-dialog')
  await dialog.getByRole('button', { name: '创建任务', exact: true }).click()
  await expect(dialog.getByRole('alert')).toBeVisible()
  await expect(page.getByTestId('task-title-input')).toHaveValue('保留的 RPC 草稿')
  await dialog.getByRole('button', { name: '重新尝试', exact: true }).click()
  await expect(dialog).toBeHidden()
  expect(attempts).toHaveLength(2)
  expect(attempts[0]?.idempotencyKey).toBeTruthy()
  expect(attempts[1]?.idempotencyKey).toBe(attempts[0]?.idempotencyKey)
  expect(attempts[1]?.taskId).toBe(attempts[0]?.taskId)
  await expect(page.getByRole('textbox', { name: '任务标题', exact: true })).toHaveValue('保留的 RPC 草稿')
  assertProtocol(seen)
  await evidence(info, 'retry-requests', seen)
})

test('实际 Host：WatchQueries 失败后仍只重连查询 RPC，退出页面后停止旧连接', async ({ page }, info) => {
  const { rpc, board, taskId } = await seed()
  const seen = requests(page)
  let subscriptions = 0
  await page.route('**/kanban.v1.QueryService/WatchQueries', async route => {
    subscriptions += 1
    if (subscriptions === 1) await route.fulfill(unavailable())
    else await route.fallback()
  })
  await page.goto(`/app/boards/${board}/list?task=${taskId}`)
  await expect(page.getByRole('textbox', { name: '任务标题', exact: true })).toHaveValue('needle 初始任务')
  await expect.poll(() => subscriptions).toBeGreaterThan(1)
  const current = await rpc.business.getTask({ taskId })
  await rpc.business.updateTask({ taskId, title: '重连后可见', expectedLockVersion: current.data!.lockVersion })
  await expect(page.getByRole('textbox', { name: '任务标题', exact: true })).toHaveValue('重连后可见')
  await page.goto('about:blank')
  const atUnmount = subscriptions
  await rpc.business.createComment({ taskId, body: '卸载后变化', author: 'writer' })
  await page.waitForTimeout(300)
  expect(subscriptions).toBe(atUnmount)
  assertProtocol(seen)
  await evidence(info, 'reconnect-requests', seen)
})

test('实际 Host：浏览器上传和下载 5 MiB 附件，超过旧 JSON 上限仍保留原始内容', async ({ page }, info) => {
  const { board, taskId } = await seed()
  const seen = requests(page)
  await page.goto(`/app/boards/${board}/list?task=${taskId}`)
  await expect(page.getByTestId('task-inspector')).toBeVisible()
  await page.locator('summary').filter({ hasText: /^标签$/ }).click()
  const content = Buffer.alloc(5 * 1024 * 1024, 0xab)
  const filename = 'grpc-中文附件.bin'
  await page.getByTestId('attachment-file').setInputFiles({ name: filename, mimeType: 'application/octet-stream', buffer: content })
  await expect(page.getByTestId('attachment-row')).toContainText(filename)
  const downloadEvent = page.waitForEvent('download')
  await page.getByTestId('attachment-download').click()
  const download = await downloadEvent
  expect(download.suggestedFilename()).toBe(filename)
  const path = await download.path()
  if (!path) throw new Error('浏览器没有保存下载内容')
  const { readFile } = await import('node:fs/promises')
  expect((await readFile(path)).equals(content)).toBe(true)
  expect(seen.some(request => request.url.endsWith('/FinishFileUpload'))).toBe(true)
  expect(seen.some(request => request.url.endsWith('/DownloadFile'))).toBe(true)
  assertProtocol(seen)
  await evidence(info, 'attachment-requests', seen)
})
