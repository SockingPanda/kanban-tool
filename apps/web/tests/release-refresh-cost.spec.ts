import { randomUUID } from 'node:crypto'
import { readFile, writeFile } from 'node:fs/promises'
import { expect, test, type Page, type Request } from '@playwright/test'
import { DtoApiCreateTaskStatus } from '../src/generated/rpc/kanban/v1/dto_pb'
import { createRpcClients } from '../src/lib/rpc/client'

const baseUrl = process.env.KANBAN_RELEASE_BASE_URL
const hostPid = process.env.KANBAN_RELEASE_HOST_PID

function track(page: Page) {
  const calls: { method: string; at: number }[] = []
  const active = new Set<Request>()
  let last = Date.now()
  page.on('request', request => {
    const path = new URL(request.url()).pathname
    if (!path.startsWith('/kanban.v1.KanbanService/')) return
    calls.push({ method: path.split('/').at(-1)!, at: Date.now() })
    active.add(request)
    last = Date.now()
  })
  const finished = (request: Request) => { if (active.delete(request)) last = Date.now() }
  page.on('requestfinished', finished)
  page.on('requestfailed', finished)
  return {
    calls,
    async settle() {
      // 等待真实 unary 完成，再留出一个有界静默窗口；持续订阅不参与 network-idle。
      await expect.poll(() => active.size === 0 && Date.now() - last > 150, { timeout: 10_000 }).toBe(true)
    },
  }
}

async function rss() {
  if (!hostPid || !/^\d+$/.test(hostPid)) return null
  const status = await readFile(`/proc/${hostPid}/status`, 'utf8')
  return {
    rssKiB: Number(/^VmRSS:\s+(\d+)/m.exec(status)?.[1]),
    peakRssKiB: Number(/^VmHWM:\s+(\d+)/m.exec(status)?.[1]),
    load: (await readFile('/proc/loadavg', 'utf8')).trim(),
  }
}

function counts(calls: { method: string }[]) {
  const result: Record<string, number> = {}
  for (const call of calls) result[call.method] = (result[call.method] ?? 0) + 1
  return result
}

function percentile(values: number[], fraction: number) {
  const sorted = [...values].sort((a, b) => a - b)
  return sorted[Math.ceil(sorted.length * fraction) - 1]
}

test('实际刷新成本：跨板、同查询多消费者、非卡片突发、失败与卸载', async ({ page, context }, info) => {
  test.skip(!baseUrl, '需要隔离真实 Host')
  test.setTimeout(90_000)
  const rpc = createRpcClients(baseUrl!)
  const seed = async () => {
    const board = `cost-${randomUUID().slice(0, 8)}`
    await rpc.business.createBoard({ slug: board, name: '刷新测量' })
    const result = await rpc.business.createTask({ board, title: '测量任务', status: DtoApiCreateTaskStatus.TODO })
    if (!result.data?.id) throw new Error('缺少 task id')
    return { board, taskId: result.data.id }
  }
  const [a, b] = await Promise.all([seed(), seed()])
  const before = await rss()
  const duplicate = await context.newPage()
  const unrelated = await context.newPage()
  const pages = [page, duplicate, unrelated]
  const trackers = pages.map(track)
  const settle = async () => { await Promise.all(trackers.map(observer => observer.settle())) }
  await Promise.all(pages.map(async (view, index) => {
    const target = index === 2 ? b : a
    await view.goto(`/app/boards/${target.board}/list?task=${target.taskId}`)
    await expect(view.getByRole('textbox', { name: '任务标题', exact: true })).toHaveValue('测量任务')
    await view.getByRole('button', { name: /^讨论/ }).click()
  }))
  await settle()
  const subscribed = await rss()
  const samples: { latencyMs: number; reads: Record<string, number>[] }[] = []
  for (let index = 0; index < 5; index += 1) {
    const offsets = trackers.map(observer => observer.calls.length)
    const body = `非卡片变化-${index}-${randomUUID()}`
    const started = Date.now()
    await rpc.business.createComment({ taskId: a.taskId, body, author: '测量作者' })
    await Promise.all([page, duplicate].map(view => expect(view.getByTestId('task-discussion')).toContainText(body)))
    const latencyMs = Date.now() - started
    await expect(unrelated.getByTestId('task-discussion')).not.toContainText(body)
    await settle()
    samples.push({ latencyMs, reads: trackers.map((observer, i) => counts(observer.calls.slice(offsets[i]))) })
  }
  await Promise.all([page, duplicate].map(async view => {
    await view.getByRole('combobox', { name: '讨论排序' }).click()
    await view.getByRole('option', { name: '最新在前', exact: true }).click()
  }))
  await settle()
  const burstOffsets = trackers.map(observer => observer.calls.length)
  const burstStart = Date.now()
  const written = await Promise.all(Array.from({ length: 20 }, (_, index) => rpc.business.createComment({ taskId: a.taskId, body: `burst-${index}`, author: '突发作者' })))
  // 并发请求的提交次序由服务决定；按返回的真实时间和 ID 找到最新的已提交评论。
  const newest = written.map(result => result.data!).sort((left, right) =>
    left.createdAt === right.createdAt ? right.id.localeCompare(left.id) : left.createdAt > right.createdAt ? -1 : 1)[0]!
  await Promise.all([page, duplicate].map(async view => {
    await expect(view.getByTestId('task-discussion').locator('article').first()).toContainText(newest.body)
    await expect(view.getByTestId('task-discussion')).toContainText('1 / 3')
  }))
  const burstVisibleMs = Date.now() - burstStart
  await settle()
  const burst = trackers.map((observer, i) => counts(observer.calls.slice(burstOffsets[i])))
  const afterBurst = await rss()

  const current = await rpc.business.getTask({ taskId: a.taskId })
  const stale = current.data!.lockVersion
  await rpc.business.updateTask({ taskId: a.taskId, title: '保持提交事实', expectedLockVersion: stale })
  await expect(page.getByRole('textbox', { name: '任务标题', exact: true })).toHaveValue('保持提交事实')
  await settle()
  const failureOffsets = trackers.map(observer => observer.calls.length)
  await expect(rpc.business.updateTask({ taskId: a.taskId, title: '失败不得出现', expectedLockVersion: stale })).rejects.toThrow()
  // 失败提示允许冗余读取，必须仍显示已提交的事实。
  await page.waitForTimeout(300)
  await settle()
  await expect(page.getByRole('textbox', { name: '任务标题', exact: true })).toHaveValue('保持提交事实')
  const failedWrite = trackers.map((observer, i) => counts(observer.calls.slice(failureOffsets[i])))

  const readOffsets = trackers.map(observer => observer.calls.length)
  await Promise.all(Array.from({ length: 20 }, () => rpc.business.getTask({ taskId: a.taskId })))
  await page.waitForTimeout(300)
  await settle()
  const readOnly = trackers.map((observer, i) => observer.calls.length - readOffsets[i])
  expect(readOnly).toEqual([0, 0, 0])
  await Promise.all(pages.map(view => view.goto('about:blank')))
  const unmountedOffsets = trackers.map(observer => observer.calls.length)
  await rpc.business.createComment({ taskId: a.taskId, body: '卸载后写入', author: '测量作者' })
  await page.waitForTimeout(300)
  expect(trackers.map((observer, i) => observer.calls.length - unmountedOffsets[i])).toEqual([0, 0, 0])
  const unmounted = await rss()
  const churn = []
  for (let cycle = 0; cycle < 5; cycle += 1) {
    await Promise.all(pages.map(async (view, index) => {
      const target = index === 2 ? b : a
      await view.goto(`/app/boards/${target.board}/list?task=${target.taskId}`)
      await expect(view.getByRole('textbox', { name: '任务标题', exact: true })).toHaveValue(index === 2 ? '测量任务' : '保持提交事实')
    }))
    await settle()
    await Promise.all(pages.map(view => view.goto('about:blank')))
    await page.waitForTimeout(300)
    churn.push({ cycle, memory: await rss() })
  }
  const evidence = {
    scenario: '两个相同 board/query 页面，加一个无关 board 页面；全局提示迁移期',
    boards: [a, b], samples,
    latencyMs: { p50: percentile(samples.map(sample => sample.latencyMs), 0.5), p95: percentile(samples.map(sample => sample.latencyMs), 0.95), max: Math.max(...samples.map(sample => sample.latencyMs)) },
    burstWrites: 20, burstVisibleMs, burst, failedWrite, readOnly,
    memory: { before, subscribed, afterBurst, unmounted, churn },
  }
  const path = info.outputPath('refresh-cost.json')
  await writeFile(path, JSON.stringify(evidence, null, 2) + '\n')
  await info.attach('refresh-cost', { path, contentType: 'application/json' })
  await duplicate.close()
  await unrelated.close()
})
