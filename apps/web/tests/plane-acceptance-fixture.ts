import { readFileSync } from "node:fs"

import type { Page, Route } from "@playwright/test"

import type { WebRuntimeConfig } from "../src/lib/runtime"
import { installPersistentSse } from "./runtime-fixture"

const runtime = JSON.parse(
  readFileSync(new URL("../src/lib/api/generated/fixtures/runtime-web-config-output.valid.json", import.meta.url), "utf8"),
) as WebRuntimeConfig
const health = JSON.parse(
  readFileSync(new URL("../src/lib/api/generated/fixtures/api-health-response.valid.json", import.meta.url), "utf8"),
) as unknown

const statuses = ["triage", "todo", "scheduled", "ready", "running", "blocked", "review", "done", "archived"] as const
type TaskStatus = (typeof statuses)[number]

export type PlaneAcceptanceProject = {
  readonly id: string
  readonly slug: string
  readonly name: string
}

export const planeAcceptanceProjects: readonly PlaneAcceptanceProject[] = [
  { id: "b_default", slug: "default", name: "Default project" },
  { id: "b_agent-first", slug: "agent-first", name: "Agent-first project" },
]

export type PlaneAcceptanceFixture = {
  readonly apiRequests: string[]
}

function projectForSlug(slug: string): PlaneAcceptanceProject | undefined {
  return planeAcceptanceProjects.find((project) => project.slug === slug)
}

function projectRows() {
  return planeAcceptanceProjects.map((project) => ({
    id: project.id,
    slug: project.slug,
    name: project.name,
    description: `${project.name} fixture`,
    created_at: 1,
    updated_at: 2,
    archived_at: null,
  }))
}

function columnsFor(project: PlaneAcceptanceProject) {
  return statuses.map((status, index) => ({
    id: `col_${project.slug}_${status}`,
    board_id: project.id,
    status,
    title: status[0]?.toUpperCase() + status.slice(1),
    position: index + 1,
    hidden: status === "archived",
    wip_limit: null,
    created_at: 1,
    updated_at: 2,
  }))
}

function taskFor(project: PlaneAcceptanceProject, status: TaskStatus, position: number) {
  return {
    id: `t_${project.slug}_${status}`,
    board_id: project.id,
    board_slug: project.slug,
    ref: `${project.slug}#${position}`,
    seq: position,
    title: `${project.name} ${status} task`,
    description: "A canonical task fixture for Plane acceptance.",
    status,
    status_reason: null,
    assignee: "playwright-agent",
    priority: 1,
    position,
    scheduled_at: null,
    due_at: null,
    created_by: "playwright",
    created_at: 1,
    updated_at: 2,
    started_at: null,
    completed_at: null,
    archived_at: null,
    claim_owner: null,
    claim_expires_at: null,
    last_heartbeat_at: null,
    current_run_id: null,
    retry_count: 0,
    max_retries: null,
    result_summary: null,
    result: null,
    metadata: {},
    lock_version: 1,
    dependency_blocked: false,
    unfinished_parent_count: 0,
    execution_plan_state: "planned",
    required_step_count: 1,
    completed_required_step_count: 0,
    optional_step_count: 0,
    labels: [],
  }
}

function tasksFor(project: PlaneAcceptanceProject) {
  return [taskFor(project, "ready", 1), taskFor(project, "todo", 2)]
}

async function fulfillJson(route: Route, payload: unknown, status = 200): Promise<void> {
  await route.fulfill({
    status,
    contentType: "application/json",
    body: JSON.stringify(payload),
  })
}

/**
 * Read-only multi-project harness for the production Plane shell acceptance lane.
 * It deliberately serves generated response shapes and leaves mutation endpoints
 * unhandled so this lane cannot accidentally become a second write path.
 */
export async function installPlaneAcceptanceFixture(page: Page): Promise<PlaneAcceptanceFixture> {
  await page.route("**/app/runtime.json", async (route) => {
    await fulfillJson(route, { ...runtime, defaultBoard: "default" })
  })
  await page.route("**/health", async (route) => {
    if (new URL(route.request().url()).pathname !== "/health") {
      await route.fallback()
      return
    }
    await fulfillJson(route, health)
  })
  await installPersistentSse(page)

  const apiRequests: string[] = []
  await page.route("**/api/v1/**", async (route) => {
    const url = new URL(route.request().url())
    apiRequests.push(`${route.request().method()} ${url.pathname}${url.search}`)

    if (url.pathname === "/api/v1/boards") {
      await fulfillJson(route, { data: projectRows() })
      return
    }

    const columnsMatch = url.pathname.match(/^\/api\/v1\/boards\/([^/]+)\/columns$/)
    if (columnsMatch) {
      const project = projectForSlug(decodeURIComponent(columnsMatch[1] ?? ""))
      if (project === undefined) {
        await fulfillJson(route, { error: { code: "not_found", message: "fixture project not found" } }, 404)
        return
      }
      await fulfillJson(route, { data: columnsFor(project) })
      return
    }

    const statusMatch = url.pathname.match(/^\/api\/v1\/boards\/([^/]+)\/tasks\/by-status$/)
    if (statusMatch) {
      const project = projectForSlug(decodeURIComponent(statusMatch[1] ?? ""))
      const status = url.searchParams.get("status") as TaskStatus | null
      const limit = Number(url.searchParams.get("limit") ?? 1000)
      const offset = Number(url.searchParams.get("offset") ?? 0)
      const validStatus = status !== null && statuses.includes(status)
      const tasks = project !== undefined && validStatus && offset === 0
        ? status === "ready" || status === "todo" ? [taskFor(project, status, status === "ready" ? 1 : 2)] : []
        : []
      await fulfillJson(route, {
        data: { statuses: [{ status: validStatus ? status : "ready", tasks, page: { limit, offset, total: tasks.length } }] },
        meta: { limit, offset },
      })
      return
    }

    const listMatch = url.pathname.match(/^\/api\/v1\/boards\/([^/]+)\/tasks$/)
    if (listMatch && route.request().method() === "GET") {
      const project = projectForSlug(decodeURIComponent(listMatch[1] ?? ""))
      if (project === undefined) {
        await fulfillJson(route, { error: { code: "not_found", message: "fixture project not found" } }, 404)
        return
      }
      const query = (url.searchParams.get("q") ?? "").trim().toLocaleLowerCase()
      const allTasks = tasksFor(project)
      const filtered = query.length === 0
        ? allTasks
        : allTasks.filter((task) => `${task.title} ${task.ref}`.toLocaleLowerCase().includes(query))
      const limit = Number(url.searchParams.get("limit") ?? 100)
      const offset = Number(url.searchParams.get("offset") ?? 0)
      const tasks = filtered.slice(offset, offset + limit)
      await fulfillJson(route, { data: tasks, meta: { limit, offset, total: filtered.length } })
      return
    }

    const mapMatch = url.pathname.match(/^\/api\/v1\/boards\/([^/]+)\/task-map$/)
    if (mapMatch) {
      const project = projectForSlug(decodeURIComponent(mapMatch[1] ?? ""))
      if (project === undefined) {
        await fulfillJson(route, { error: { code: "not_found", message: "fixture project not found" } }, 404)
        return
      }
      const task = taskFor(project, "ready", 1)
      await fulfillJson(route, {
        data: {
          nodes: [{ task, role: "active", context_only: false }],
          edges: [],
          meta: {
            depth: 0,
            context_depth: Number(url.searchParams.get("context_depth") ?? 1),
            generated_at: 1,
            node_count: 1,
            edge_count: 0,
            truncated: false,
            active_statuses: ["ready", "running", "review", "blocked"],
            active_only: true,
            include_done_context: url.searchParams.get("include_done_context") === "true",
            include_archived_context: false,
            hide_isolated: url.searchParams.get("hide_isolated") === "true",
            limit_nodes: Number(url.searchParams.get("limit_nodes") ?? 240),
          },
        },
      })
      return
    }

    if (url.pathname === "/api/v1/events") {
      await fulfillJson(route, { data: [], meta: { next_after: 0 } })
      return
    }

    const taskMatch = url.pathname.match(/^\/api\/v1\/tasks\/([^/]+)$/)
    if (taskMatch && route.request().method() === "GET") {
      const taskId = decodeURIComponent(taskMatch[1] ?? "")
      const task = planeAcceptanceProjects
        .flatMap((project) => tasksFor(project))
        .find((candidate) => candidate.id === taskId)
      if (task === undefined) {
        await fulfillJson(route, { error: { code: "not_found", message: "fixture task not found" } }, 404)
        return
      }
      await fulfillJson(route, { data: task })
      return
    }

    const attachmentsMatch = url.pathname.match(/^\/api\/v1\/tasks\/([^/]+)\/attachments$/)
    if (attachmentsMatch && route.request().method() === "GET") {
      await fulfillJson(route, { data: [] })
      return
    }

    await fulfillJson(route, { error: { code: "not_found", message: "fixture route not found" } }, 404)
  })

  return { apiRequests }
}
