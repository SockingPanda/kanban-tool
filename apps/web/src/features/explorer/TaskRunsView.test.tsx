import { renderToStaticMarkup } from "react-dom/server"
import { describe, expect, test, vi } from "vitest"

import type { ApiListRunsResponseContract } from "../../lib/api/generated/contracts/api-list-runs-response"
import { ExplorerReadError, type TaskRunsReadModel } from "../../lib/api/explorer-read-model"
import { TaskRunsPresentation, type TaskRunsReadState } from "./TaskRunsView"

type Run = ApiListRunsResponseContract["data"][number]

function run(id: string, hasLog: boolean): Run {
  return {
    id,
    task_id: "t_1",
    status: "succeeded",
    worker_profile: "worker",
    worker_pid: null,
    claim_owner: "owner",
    started_at: 1,
    finished_at: 2,
    exit_code: 0,
    summary: "done",
    error: null,
    has_log: hasLog,
    metadata: {},
  }
}

const log = { run_id: "r_log", content: "hello from run log", truncated: false }
const readyModel: TaskRunsReadModel = {
  taskId: "t_1",
  runs: [run("r_no_log", false), run("r_log", true)],
  selectedRunId: "r_log",
  log,
}

const ready: TaskRunsReadState = { data: readyModel, loading: false, error: null }

describe("TaskRunsView", () => {
  test("renders no-task, loading and empty states", () => {
    const noTask = renderToStaticMarkup(<TaskRunsPresentation locale="zh" taskId={null} state={{ data: null, loading: false, error: null }} onRetry={vi.fn()} />)
    const loading = renderToStaticMarkup(<TaskRunsPresentation locale="zh" taskId="t_1" state={{ data: null, loading: true, error: null }} onRetry={vi.fn()} />)
    const empty = renderToStaticMarkup(<TaskRunsPresentation locale="zh" taskId="t_1" state={{ data: { ...readyModel, runs: [], selectedRunId: null, log: null }, loading: false, error: null }} onRetry={vi.fn()} />)

    expect(noTask).toContain('data-testid="runs-no-task"')
    expect(loading).toContain('data-testid="runs-loading"')
    expect(empty).toContain('data-testid="runs-empty"')
  })

  test("renders error and offline states with retry affordance", () => {
    const retry = vi.fn()
    const error = renderToStaticMarkup(<TaskRunsPresentation locale="zh" taskId="t_1" state={{ data: null, loading: false, error: new ExplorerReadError("http", "runs failed") }} onRetry={retry} />)
    const offline = renderToStaticMarkup(<TaskRunsPresentation locale="en" taskId="t_1" state={{ data: null, loading: false, error: new ExplorerReadError("offline", "offline") }} onRetry={retry} />)

    expect(error).toContain('data-testid="runs-error"')
    expect(offline).toContain('data-testid="runs-offline"')
    expect(offline).toContain("You are offline")
    expect(error).toContain("重试")
  })

  test("renders ready run rows and the first available log without clickable rows", () => {
    const markup = renderToStaticMarkup(<TaskRunsPresentation locale="zh" taskId="t_1" state={ready} onRetry={vi.fn()} />)

    expect(markup).toContain('data-testid="runs-ready"')
    expect(markup).toContain('data-testid="run-row"')
    expect(markup).toContain("r_no_log")
    expect(markup).toContain("hello from run log")
    expect(markup).toMatch(/<li[^>]*data-testid="run-row"[^>]*class="[^"]*x92x3c3[^"]*"/)
    expect(markup).not.toContain("<span><p>")
    expect(markup).not.toContain("<button")
    expect(markup).not.toContain("<a ")
    expect(markup).toContain('id="runs-heading"')
    expect(markup).not.toContain("<h1")
  })

  test("renders no-log ready state and English labels", () => {
    const noLog: TaskRunsReadState = { data: { ...readyModel, selectedRunId: null, log: null }, loading: false, error: null }
    const markup = renderToStaticMarkup(<TaskRunsPresentation locale="en" taskId="t_1" state={noLog} onRetry={vi.fn()} />)

    expect(markup).toContain('data-testid="runs-no-log"')
    expect(markup).toContain("No log available for the selected task.")
    expect(markup).toContain("Runs")
  })
})
