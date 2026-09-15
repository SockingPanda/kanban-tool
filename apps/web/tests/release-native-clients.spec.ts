import { execFile, spawn } from 'node:child_process'
import { randomUUID } from 'node:crypto'
import { writeFile } from 'node:fs/promises'
import { createInterface } from 'node:readline'
import { promisify } from 'node:util'
import { expect, test } from '@playwright/test'

const execute = promisify(execFile)
const baseUrl = process.env.KANBAN_RELEASE_BASE_URL
const cliBinary = process.env.KANBAN_RELEASE_CLI
const mcpBinary = process.env.KANBAN_RELEASE_MCP

async function cli(board: string, ...args: string[]) {
  if (!baseUrl || !cliBinary) throw new Error('需要真实 Host 与 KANBAN_RELEASE_CLI 候选路径')
  const result = await execute(cliBinary, ['--json', '--server-url', baseUrl, '--board', board, '--actor', 'CLI 原生作者', ...args], { timeout: 10_000 })
  return JSON.parse(result.stdout) as { data: { id: string; lock_version: number } }
}

async function mcpComment(board: string, taskId: string, body: string) {
  if (!baseUrl || !mcpBinary) throw new Error('需要真实 Host 与 KANBAN_RELEASE_MCP 候选路径')
  const child = spawn(mcpBinary, [], {
    env: { ...process.env, KANBAN_SERVER_URL: baseUrl, KB_BOARD: board, KANBAN_ACTOR: 'MCP 原生作者' },
    stdio: ['pipe', 'pipe', 'pipe'],
  })
  const lines = createInterface({ input: child.stdout })
  const pending = new Map<number, (value: Record<string, unknown>) => void>()
  let sequence = 0
  lines.on('line', line => {
    const response = JSON.parse(line) as Record<string, unknown>
    if (typeof response.id === 'number') pending.get(response.id)?.(response)
  })
  const request = (method: string, params: unknown) => new Promise<Record<string, unknown>>((resolve, reject) => {
    const id = ++sequence
    const timer = setTimeout(() => { pending.delete(id); reject(new Error(`MCP ${method} 超时`)) }, 10_000)
    pending.set(id, response => {
      clearTimeout(timer)
      pending.delete(id)
      if (response.error) reject(new Error(JSON.stringify(response.error)))
      else resolve(response)
    })
    child.stdin.write(JSON.stringify({ jsonrpc: '2.0', id, method, params }) + '\n')
  })
  try {
    await request('initialize', { protocolVersion: '2024-11-05', capabilities: {}, clientInfo: { name: 'grpc-browser-proof', version: '1' } })
    child.stdin.write(JSON.stringify({ jsonrpc: '2.0', method: 'notifications/initialized' }) + '\n')
    const response = await request('tools/call', { name: 'comment_create', arguments: { task_ref: taskId, body, idempotency_key: randomUUID() } })
    expect((response.result as { isError?: boolean }).isError).not.toBe(true)
    return response.result
  } finally {
    lines.close()
    child.stdin.end()
    child.kill('SIGTERM')
  }
}

test('实际 CLI 与 MCP 的原生写入通过 gRPC-Web 出现在 Atlas', async ({ page }, info) => {
  test.skip(!cliBinary || !mcpBinary, '原生入口验收需要两个候选二进制')
  const board = `native-${randomUUID().slice(0, 8)}`
  await cli(board, 'board', 'create', board, '--name', '原生入口验收')
  const created = await cli(board, 'task', 'create', 'CLI 初始任务', '--status', 'todo')
  const taskId = created.data.id
  const requests: { path: string; method: string; contentType?: string }[] = []
  page.on('request', request => requests.push({ path: new URL(request.url()).pathname, method: request.method(), contentType: request.headers()['content-type'] }))
  await page.goto(`/app/boards/${board}/list?task=${taskId}`)
  const title = page.getByRole('textbox', { name: '任务标题', exact: true })
  await expect(title).toHaveValue('CLI 初始任务')
  const cliStart = Date.now()
  await cli(board, 'task', 'update', taskId, '--title', 'CLI 外部更新已可见')
  await expect(title).toHaveValue('CLI 外部更新已可见')
  const cliVisibleMs = Date.now() - cliStart
  await page.getByRole('button', { name: /^讨论/ }).click()
  const mcpStart = Date.now()
  const comment = await mcpComment(board, taskId, 'MCP 外部评论已可见')
  await expect(page.getByTestId('task-discussion')).toContainText('MCP 外部评论已可见')
  const mcpVisibleMs = Date.now() - mcpStart
  expect(requests.filter(request => request.path.startsWith('/api/v1/'))).toEqual([])
  const rpc = requests.filter(request => request.path.startsWith('/kanban.v1.'))
  expect(rpc.length).toBeGreaterThan(0)
  expect(rpc.every(request => request.method === 'POST' && request.contentType === 'application/grpc-web+proto')).toBe(true)
  const path = info.outputPath('native-to-web.json')
  await writeFile(path, JSON.stringify({ board, taskId, cliBinary, mcpBinary, cliVisibleMs, mcpVisibleMs, comment, requests }, null, 2) + '\n')
  await info.attach('native-to-web', { path, contentType: 'application/json' })
  await page.screenshot({ path: info.outputPath('native-to-web.png'), fullPage: true })
})
