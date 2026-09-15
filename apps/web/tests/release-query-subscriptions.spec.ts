import { randomUUID } from 'node:crypto'
import { readFile, writeFile } from 'node:fs/promises'
import { expect, test, type Page, type Request } from '@playwright/test'
import { DtoApiCreateTaskStatus } from '../src/generated/rpc/kanban/v1/dto_pb'
import { createRpcClients } from '../src/lib/rpc/client'
import { actorHeaders } from '../src/lib/rpc/errors'

const baseUrl = process.env.KANBAN_RELEASE_BASE_URL
const hostPid = process.env.KANBAN_RELEASE_HOST_PID

function track(page: Page) {
  const calls: { method: string; at: number }[] = []
  const active = new Set<Request>()
  const queryStreams: Request[] = []
  const activeQueries = new Set<Request>()
  const queryDocuments = new Map<Request, number>()
  let documentGeneration = 0
  let documentNavigation = false
  const legacyStreams: string[] = []
  let last = Date.now()
  page.on('request', request => {
    if (request.isNavigationRequest() && request.frame() === page.mainFrame()) documentNavigation = true
    const path = new URL(request.url()).pathname
    if (path.includes('WorkspaceService/WatchChanges')) legacyStreams.push(path)
    if (path.includes('QueryService/WatchQueries')) { queryStreams.push(request); activeQueries.add(request); queryDocuments.set(request, documentGeneration) }
    if (!path.startsWith('/kanban.v1.KanbanService/')) return
    calls.push({ method: path.split('/').at(-1)!, at: Date.now() })
    active.add(request)
    last = Date.now()
  })
  const finished = (request: Request) => { activeQueries.delete(request); if (active.delete(request)) last = Date.now() }
  page.on('requestfinished', finished)
  page.on('requestfailed', finished)
  page.on('framenavigated', frame => {
    if (frame === page.mainFrame() && (documentNavigation || frame.url() === 'about:blank')) {
      documentGeneration += 1
      documentNavigation = false
    }
  })
  return {
    calls, queryStreams, legacyStreams,
    // Playwright 在 document 销毁时可能不补流的结束事件；保留旧记录，当前 dataSource 单独计数。
    get activeDocumentQueries() { return [...activeQueries].filter(request => queryDocuments.get(request) === documentGeneration).length },
    queryLifetimes: () => ({ documents: documentGeneration, unsettledRequestRecords: activeQueries.size }),
    async settle() {
      // 等待真实 unary 完成，再留出一个有界静默窗口；持续订阅不参与 network-idle。
      await expect.poll(() => active.size === 0 && Date.now() - last > 150, { timeout: 10_000 }).toBe(true)
    },
  }
}

/** Host 全局上限为 16 条流；所有页面卸载后同时取得 16 个许可，证明旧 document 没有遗留订阅。 */
async function releasedQueryCapacity(rpc: ReturnType<typeof createRpcClients>): Promise<number> {
  const controllers = Array.from({ length: 16 }, () => new AbortController())
  const streams = controllers.map((controller, index) => rpc.query.watchQueries({
    protocolVersion: 1,
    queries: [{ clientQueryId: `capacity-${index}`, projectionVersion: 1, query: { case: 'getHealth', value: {} } }],
  }, { signal: controller.signal, timeoutMs: 10_000 })[Symbol.asyncIterator]())
  try {
    const results = await Promise.allSettled(streams.map(stream => stream.next()))
    console.info('query-capacity-probe', { accepted: results.filter(result => result.status === 'fulfilled').length, rejected: results.filter(result => result.status === 'rejected').length })
    expect(results.filter(result => result.status === 'rejected')).toEqual([])
    return results.filter(result => result.status === 'fulfilled' && !result.value.done && result.value.value.body.case === 'begin').length
  } finally {
    for (const controller of controllers) controller.abort()
    await Promise.allSettled(streams.map(stream => stream.return?.()))
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

test('G07 完整查询流成本：跨板、同查询多消费者、非卡片突发、失败与卸载', async ({ page, context }, info) => {
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
  await expect.poll(() => trackers.map(observer => observer.activeDocumentQueries)).toEqual([1, 1, 1])
  expect(trackers.flatMap(observer => observer.legacyStreams)).toEqual([])
  expect(trackers.flatMap(observer => observer.calls)).toEqual([])
  const subscribed = await rss()
  const samples: { latencyMs: number; reads: Record<string, number>[] }[] = []
  for (let index = 0; index < 5; index += 1) {
    const offsets = trackers.map(observer => observer.calls.length)
    const streamCounts = trackers.map(observer => observer.queryStreams.length)
    const body = `非卡片变化-${index}-${randomUUID()}`
    const started = Date.now()
    await rpc.business.createComment({ taskId: a.taskId, body, author: '测量作者' })
    await Promise.all([page, duplicate].map(view => expect(view.getByTestId('task-discussion')).toContainText(body)))
    const latencyMs = Date.now() - started
    await expect(unrelated.getByTestId('task-discussion')).not.toContainText(body)
    await settle()
    const reads = trackers.map((observer, i) => counts(observer.calls.slice(offsets[i])))
    expect(reads).toEqual([{}, {}, {}])
    expect(trackers.map(observer => observer.queryStreams.length)).toEqual(streamCounts)
    samples.push({ latencyMs, reads })
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
  const comments = written.map(result => {
    const comment = result.data
    if (!comment || comment.id === undefined || comment.body === undefined || comment.createdAt === undefined) throw new Error('评论响应缺少排序与展示字段')
    return { id: comment.id, body: comment.body, createdAt: comment.createdAt }
  })
  const newest = comments.sort((left, right) =>
    left.createdAt === right.createdAt ? right.id.localeCompare(left.id) : left.createdAt > right.createdAt ? -1 : 1)[0]!
  await Promise.all([page, duplicate].map(async view => {
    await expect(view.getByTestId('task-discussion').locator('article').first()).toContainText(newest.body)
    await expect(view.getByTestId('task-discussion')).toContainText('1 / 3')
  }))
  const burstVisibleMs = Date.now() - burstStart
  await settle()
  const burst = trackers.map((observer, i) => counts(observer.calls.slice(burstOffsets[i])))
  expect(burst).toEqual([{}, {}, {}])
  const afterBurst = await rss()

  const current = await rpc.business.getTask({ taskId: a.taskId })
  const stale = current.data!.lockVersion
  await rpc.business.updateTask({ taskId: a.taskId, title: '保持提交事实', expectedLockVersion: stale })
  await expect(page.getByRole('textbox', { name: '任务标题', exact: true })).toHaveValue('保持提交事实')
  await settle()
  const failureOffsets = trackers.map(observer => observer.calls.length)
  await expect(rpc.business.updateTask({ taskId: a.taskId, title: '失败不得出现', expectedLockVersion: stale })).rejects.toThrow()
  // 失败写入没有结果变化，不触发页面 unary 回读。
  await page.waitForTimeout(300)
  await settle()
  await expect(page.getByRole('textbox', { name: '任务标题', exact: true })).toHaveValue('保持提交事实')
  const failedWrite = trackers.map((observer, i) => counts(observer.calls.slice(failureOffsets[i])))

  expect(failedWrite).toEqual([{}, {}, {}])
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
  await expect(async () => expect(await releasedQueryCapacity(rpc)).toBe(16)).toPass({ timeout: 10_000 })
  const evidence = {
    scenario: 'G07：两个相同 board/query 页面，加一个无关 board 页面；完整 QueryResult 流',
    boards: [a, b], samples,
    latencyMs: { p50: percentile(samples.map(sample => sample.latencyMs), 0.5), p95: percentile(samples.map(sample => sample.latencyMs), 0.95), max: Math.max(...samples.map(sample => sample.latencyMs)) },
    burstWrites: 20, burstVisibleMs, burst, failedWrite, readOnly,
    memory: { before, subscribed, afterBurst, unmounted, churn },
    queryLifetimes: trackers.map(observer => observer.queryLifetimes()), releasedHostQueryPermits: 16,
  }
  const path = info.outputPath('query-refresh-cost.json')
  await writeFile(path, JSON.stringify(evidence, null, 2) + '\n')
  await info.attach('query-refresh-cost', { path, contentType: 'application/json' })
  await duplicate.close()
  await unrelated.close()
})

test('G07 实际完整读模型：分页补位、草稿焦点、Map、Runs、Events 和切板释放', async ({ page, request }, info) => {
  test.skip(!baseUrl, '需要隔离真实 Host')
  test.setTimeout(90_000)
  const rpc = createRpcClients(baseUrl!)
  const board = `query-${randomUUID().slice(0, 8)}`
  const other = `query-${randomUUID().slice(0, 8)}`
  await rpc.business.createBoard({ slug: board, name: '查询订阅验收' })
  await rpc.business.createBoard({ slug: other, name: '切板验收' })
  const tasks = await Promise.all(Array.from({ length: 11 }, (_, index) => rpc.business.createTask({ board, title: `query-${index}`, description: '完整查询流验收', status: DtoApiCreateTaskStatus.TODO })))
  const taskId = tasks[0].data!.id
  const observer = track(page)
  const errors: string[] = []
  page.on('pageerror', error => errors.push(error.message))
  const runtime = await (await request.get(`${baseUrl}/app/runtime.json`)).json()
  const manifest = await (await request.get(`${baseUrl}/app/manifest.json`)).json()
  expect(runtime.webBuildId).toBe(manifest.buildId)
  await page.goto(`/app/boards/${board}/list?page=2&limit=10&sort=seq&task=${taskId}`)
  await expect(page.locator('main')).toHaveAttribute('data-runtime-web-build-id', runtime.webBuildId)
  await expect(page.getByTestId('task-row')).toHaveCount(1)
  const title = page.getByRole('textbox', { name: '任务标题', exact: true })
  await expect(title).toHaveValue('query-0')
  await page.getByRole('button', { name: /^讨论/ }).click()
  await title.fill('保留未保存草稿')
  await expect(title).toBeFocused()
  const current = await rpc.business.getTask({ taskId })
  await rpc.business.updateTask({ taskId, title: 'query-外部标题', expectedLockVersion: current.data!.lockVersion })
  await rpc.business.createComment({ taskId, body: '草稿期间外部评论', author: '流验收' })
  await expect(page.getByTestId('task-discussion')).toContainText('草稿期间外部评论')
  await expect(title).toHaveValue('保留未保存草稿')
  await expect(title).toBeFocused()
  const added = await rpc.business.createTask({ board, title: 'query-窗口补位', description: '新成员', status: DtoApiCreateTaskStatus.TODO })
  await expect(page.getByTestId('task-row')).toHaveCount(2)
  await expect(page.getByTestId('task-list')).toContainText('query-窗口补位')
  await expect(page).toHaveURL(new RegExp(`page=2&limit=10&sort=seq&task=${taskId}`))
  // 恢复已知 canonical 值后离焦，不把未保存草稿变成测试之外的写入。
  await title.fill('query-外部标题')
  await title.press('Tab')
  await page.getByRole('button', { name: '关闭任务检查器', exact: true }).click()
  await observer.settle()
  expect(observer.calls).toEqual([])

  await page.goto(`/app/boards/${board}/map`)
  await expect(page.getByTestId('task-map-node')).toHaveCount(12)
  await rpc.business.updateTask({ taskId: added.data!.id, title: 'Map 完整投影更新' })
  await expect(page.getByTestId('task-map-node').filter({ hasText: 'Map 完整投影更新' })).toBeVisible()

  await rpc.business.markExecutionPlanNotRequired({ taskId, reason: '本测试只验证运行状态订阅' })
  await rpc.business.promoteTask({ taskId })
  await page.goto(`/app/boards/${board}/runs?task=${taskId}`)
  await expect(page.getByTestId('runs-empty')).toBeVisible()
  const claimed = await rpc.business.claimTask({ taskId, ttlMs: 60_000n, actor: 'query-proof' })
  await expect(page.getByTestId('run-row')).toHaveCount(1)
  await expect(page.getByTestId('run-row')).toContainText('运行中')
  await rpc.business.completeTask({ taskId, claimToken: claimed.data?.claimToken }, { headers: actorHeaders('query-proof') })
  await expect(page.getByTestId('run-row')).toContainText('成功')

  await page.goto(`/app/boards/${board}/events`)
  await expect(page.getByTestId('event-row').first()).toBeVisible()
  await rpc.business.createComment({ taskId, body: '近期事件窗口新增', author: 'query-events-proof' }, { headers: actorHeaders('query-events-proof') })
  await expect(page.getByTestId('event-row').filter({ hasText: 'query-events-proof' })).toBeVisible()
  await observer.settle()
  expect(observer.calls).toEqual([])
  expect(observer.legacyStreams).toEqual([])
  await expect.poll(() => observer.activeDocumentQueries).toBe(1)

  await page.goto(`/app/boards/${other}/list`)
  await expect(page.getByRole('heading', { name: '这里还没有任务', exact: true })).toBeVisible()
  await rpc.business.createComment({ taskId, body: '旧项目迟到更新', author: 'late-old-board' })
  await expect(page.getByTestId('task-inspector')).toHaveCount(0)
  await expect(page.getByTestId('task-row')).toHaveCount(0)
  await page.goto('about:blank')
  await expect.poll(() => observer.activeDocumentQueries).toBe(0)
  await expect(async () => expect(await releasedQueryCapacity(rpc)).toBe(16)).toPass({ timeout: 10_000 })
  expect(errors).toEqual([])
  const path = info.outputPath('query-full-models.json')
  await writeFile(path, JSON.stringify({ board, other, taskId, buildId: runtime.webBuildId, unaryCalls: observer.calls, queryConnections: observer.queryStreams.length, queryLifetimes: observer.queryLifetimes(), releasedHostQueryPermits: 16, errors }, null, 2) + '\n')
  await info.attach('query-full-models', { path, contentType: 'application/json' })
})
