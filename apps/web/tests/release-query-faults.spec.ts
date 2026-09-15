import { randomUUID } from "node:crypto"

import { expect, test, type Page } from "@playwright/test"

import { DtoApiCreateTaskStatus } from "../src/generated/rpc/kanban/v1/dto_pb"
import { createRpcClients } from "../src/lib/rpc/client"
import { installQueryProbe } from "./release-query-probe"

const baseURL = process.env.KANBAN_RELEASE_BASE_URL
const probes = new WeakMap<Page, Awaited<ReturnType<typeof installQueryProbe>>>()
test.afterEach(async ({ page }, info) => {
  const probe = probes.get(page)
  if (probe) await info.attach("query-wire", { body: JSON.stringify({ requests: probe.requests, actions: probe.actions, frames: probe.frames.map(item => ({ connection: item.connection, method: item.definition?.query.case, kind: item.frame.body.case })) }, (_key, value) => typeof value === "bigint" ? String(value) : value), contentType: "application/json" })
})

async function seed(description = "故障恢复验收") {
  const rpc = createRpcClients(baseURL!)
  const board = "fault-" + randomUUID().slice(0, 8)
  await rpc.business.createBoard({ slug: board, name: "查询故障注入" })
  const task = await rpc.business.createTask({ board, title: "完整提交的任务", description, status: DtoApiCreateTaskStatus.TODO })
  if (!task.data?.id) throw new Error("种子缺少 task id")
  return { rpc, board, taskId: task.data.id }
}

function trackUnary(page: Page) {
  const requests: string[] = []
  page.on("request", request => { if (new URL(request.url()).pathname.startsWith("/kanban.v1.KanbanService/")) requests.push(request.url()) })
  return requests
}

test("实际 Host：途中 snapshot EOF 不提交半个详情，重新订阅后完整恢复", async ({ page }, info) => {
  test.skip(!baseURL, "需要隔离真实 Host")
  test.setTimeout(45_000)
  const { board, taskId } = await seed("多块完整说明。".repeat(4_000))
  const unary = trackUnary(page)
  const probe = await installQueryProbe(page)
  probes.set(page, probe)
  let release!: () => void
  const held = new Promise<void>(resolve => { release = resolve })
  let armed = true
  let interrupted = 0
  probe.onFrame(async ({ definition, frame }) => {
    if (armed && definition?.query.case === "getTask" && frame.body.case === "chunk" && frame.body.value.index === 1) {
      interrupted += 1
      await held
      return "eof"
    }
    return "pass"
  })
  await page.goto(`/app/boards/${board}/list`)
  await expect(page.getByTestId("task-list")).toContainText("完整提交的任务")
  await page.getByTestId("task-row").filter({ hasText: "完整提交的任务" }).click()
  await expect(page).toHaveURL(new RegExp(`task=${taskId}`))
  await expect.poll(() => interrupted).toBeGreaterThan(0)
  await expect(page.getByRole("textbox", { name: "任务标题", exact: true })).toHaveCount(0)
  expect(probe.committed("getTask")).toBeUndefined()
  const beforeRecovery = probe.requests.length
  armed = false
  release()
  await expect(page.getByRole("textbox", { name: "任务标题", exact: true })).toHaveValue("完整提交的任务")
  await expect.poll(() => probe.requests.length).toBeGreaterThan(beforeRecovery)
  const recovery = probe.requests.slice(beforeRecovery).find(request => request.queries.some(query => query.query.case === "getTask"))
  expect(recovery?.queries.find(query => query.query.case === "getTask")?.resume).toBeUndefined()
  expect(probe.frames.filter(item => item.definition?.query.case === "getTask" && item.frame.body.case === "begin" && item.frame.body.value.snapshot).length).toBeGreaterThanOrEqual(2)
  expect(probe.committed("getTask")).toBeDefined()
  expect(unary).toEqual([])
  await info.attach("snapshot-eof", { body: JSON.stringify({ interrupted, requestsBeforeRecovery: beforeRecovery, requestsAfterRecovery: probe.requests.length, unaryReads: unary.length }), contentType: "application/json" })
})

test("实际 Host：正常流 EOF 保留 committed cursor，pageshow persisted 恢复草稿与焦点", async ({ page }, info) => {
  test.skip(!baseURL, "需要隔离真实 Host")
  test.setTimeout(45_000)
  const { rpc, board, taskId } = await seed()
  const unary = trackUnary(page)
  const probe = await installQueryProbe(page)
  probes.set(page, probe)
  await page.goto(`/app/boards/${board}/list?task=${taskId}`)
  const title = page.getByRole("textbox", { name: "任务标题", exact: true })
  await expect(title).toHaveValue("完整提交的任务")
  await page.getByRole("button", { name: /^讨论/ }).click()
  await expect(page.getByTestId("task-discussion")).toBeVisible()
  await probe.settle()
  const committed = probe.committed("listComments")
  expect(committed).toBeDefined()
  await title.fill("尚未保存的本地草稿")
  let interrupted = false
  let baselineRequestCount = 0
  probe.onFrame(({ connection, definition, frame }) => {
    if (connection === probe.requests.at(-1)?.connection && !interrupted && definition?.query.case === "listComments" && frame.body.case === "end") {
      interrupted = true
      baselineRequestCount = probe.requests.length
      return "complete"
    }
    return "pass"
  })
  await rpc.business.createComment({ taskId, body: "断线后必须完整恢复的评论", author: "故障注入" })
  await expect(page.getByTestId("task-discussion")).toContainText("断线后必须完整恢复的评论")
  expect(interrupted).toBe(true)
  const resumed = probe.requests.slice(baselineRequestCount).flatMap(request => request.queries).find(query => query.query.case === "listComments")?.resume
  expect(resumed).toEqual(committed)
  await expect(title).toHaveValue("尚未保存的本地草稿")
  await expect(title).toBeFocused()

  await page.evaluate(() => window.dispatchEvent(new PageTransitionEvent("pagehide", { persisted: true })))
  const hiddenRequestCount = probe.requests.length
  await rpc.business.createComment({ taskId, body: "恢复页面后可见的评论", author: "故障注入" })
  await page.waitForTimeout(300)
  expect(probe.requests.length).toBe(hiddenRequestCount)
  await expect(page.getByTestId("task-discussion")).not.toContainText("恢复页面后可见的评论")
  await page.evaluate(() => window.dispatchEvent(new PageTransitionEvent("pageshow", { persisted: true })))
  await expect(page.getByTestId("task-discussion")).toContainText("恢复页面后可见的评论")
  await expect(title).toHaveValue("尚未保存的本地草稿")
  await expect(title).toBeFocused()
  expect(unary).toEqual([])
  await info.attach("query-recovery", { body: JSON.stringify({ resume: { epoch: resumed?.epoch, scope: resumed?.scope, revision: String(resumed?.revision) }, lifecycle: "injected-pagehide-pageshow-persisted", hiddenRequestCount, restoredRequestCount: probe.requests.length, unaryReads: unary.length }), contentType: "application/json" })
})
