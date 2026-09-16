import { expect, test } from '@playwright/test'
import { installExplorerFixture } from './explorer-fixture'

test('完整 64 位值显示、草稿、日期与 CAS 回传经过生产 gRPC-Web 链路', async ({ page }, info) => {
  const fixture = await installExplorerFixture(page)
  const minimum = -9223372036854775808n, maximum = 9223372036854775807n, unsigned = 18446744073709551615n
  const task = { ...fixture.readyTask(), seq: maximum, position: maximum, lock_version: maximum - 2n, scheduled_at: minimum, due_at: maximum, required_step_count: maximum, completed_required_step_count: maximum - 1n }
  const writes: Record<string, unknown>[] = []
  const errors: string[] = []
  page.on('pageerror', error => errors.push(error.message))
  fixture.rpc.handle('GetTask', () => ({ data: task }))
  fixture.rpc.handle('ListTasks', call => ({ data: [task], meta: { total: 1, limit: call.query.limit ?? 100, offset: call.query.offset ?? 0 } }))
  fixture.rpc.handle('UpdateTask', call => {
    expect(call.input.expected_lock_version).toBe(task.lock_version)
    writes.push(call.input)
    Object.assign(task, Object.fromEntries(['title', 'description', 'assignee', 'priority', 'scheduled_at', 'due_at'].filter(key => Object.hasOwn(call.input, key)).map(key => [key, call.input[key]])))
    task.lock_version += 1n
    return { data: task }
  })
  const comment = (id: string, created_at: bigint) => ({ id, board_id: 'b_default', task_id: 't_ready', author: 'tester', author_type: 'user', agent_type: null, body: id, kind: 'note', created_at, metadata: { integer: unsigned, text: String(unsigned) } })
  fixture.rpc.handle('ListComments', () => ({ data: [comment('第二条', 9007199254740993n), comment('第一条', 9007199254740992n)] }))

  await page.goto('/app/boards/default/list?task=t_ready')
  await expect(page.getByTestId('task-inspector')).toBeVisible()
  await expect(page.getByTestId('task-inspector-steps')).toContainText('9223372036854775806/9223372036854775807')
  await page.getByText('运行记录与任务操作', { exact: true }).click()
  await page.getByRole('button', { name: '编辑更多属性', exact: true }).click()
  const form = page.getByTestId('inspector-edit-form')
  await expect(form.locator('[name="task-scheduled-at"]')).toHaveValue(String(minimum))
  await expect(form.locator('[name="task-due-at"]')).toHaveValue(String(maximum))
  await form.locator('[name="task-title"]').fill('精确保存后的标题')
  await form.getByRole('button', { name: '保存', exact: true }).click()
  await expect(form).toHaveCount(0)
  expect(writes[0]).toMatchObject({ expected_lock_version: maximum - 2n, scheduled_at: minimum, due_at: maximum })
  await expect(page.getByRole('textbox', { name: '任务标题', exact: true })).toHaveValue('精确保存后的标题')

  await page.getByRole('button', { name: /^讨论/ }).click()
  const discussion = page.getByTestId('task-discussion')
  await expect(discussion.locator('.comment > p')).toHaveText(['第一条', '第二条'])
  await discussion.getByText('附加信息', { exact: true }).first().click()
  await expect(discussion.locator('pre').first()).toContainText('"integer": 18446744073709551615')
  await expect(discussion.locator('pre').first()).toContainText('"text": "18446744073709551615"')
  const draft = page.getByRole('textbox', { name: '评论内容', exact: true })
  await draft.fill('保留这份草稿')
  task.completed_required_step_count = maximum
  await fixture.rpc.publish()
  await expect(draft).toHaveValue('保留这份草稿')
  await expect(draft).toBeFocused()
  await page.setViewportSize({ width: 390, height: 900 })
  await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(390)
  await page.screenshot({ path: info.outputPath('integer-mobile.png'), fullPage: true })
  await page.reload()
  await expect(page.getByRole('textbox', { name: '任务标题', exact: true })).toHaveValue('精确保存后的标题')
  expect(task.scheduled_at).toBe(minimum)
  expect(task.due_at).toBe(maximum)
  expect(errors).toEqual([])
  expect(fixture.rpc.failures).toEqual([])
})
