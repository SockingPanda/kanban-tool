import { describe, expect, test } from "vitest"

import {
  buildTaskListRequest,
  buildTaskInspectorRequests,
  buildTaskMapRequest,
  defaultTaskListQuery,
  ExplorerReadError,
  loadTaskInspector,
  parseTaskListQuery,
  serializeTaskListQuery,
} from "./explorer-read-model"
import type { WebRuntimeConfig } from "../runtime"
import type { HttpTransportResponse } from "./http-transport"
import { HttpTransportError } from "./http-transport"

const runtime = {
  apiBaseUrl: "",
  webBasePath: "/app/",
  actor: "test",
  defaultBoard: "default",
  serverVersion: "3.0.0",
  protocolVersion: "v1",
  webBuildId: "test",
} satisfies WebRuntimeConfig

describe("explorer task list URL state", () => {
  test("parses and serializes all list controls without losing them", () => {
    const query = parseTaskListQuery(
      "?status=ready&status=running&priority=1&priority=3&plan=has_steps&plan=incomplete_required_steps&q=needle&sort=-updated_at&page=3&limit=50&include_archived=true",
    )

    expect(query).toEqual({
      ...defaultTaskListQuery,
      status: ["ready", "running"],
      priority: [1, 3],
      plan: ["has_steps", "incomplete_required_steps"],
      search: "needle",
      sort: "-updated_at",
      page: 3,
      limit: 50,
      includeArchived: true,
    })
    expect(serializeTaskListQuery(query)).toBe(
      "?status=ready&status=running&priority=1&priority=3&plan=has_steps&plan=incomplete_required_steps&q=needle&sort=-updated_at&page=3&limit=50&include_archived=true",
    )
  })

  test("normalizes unsafe values back to the generated query contract defaults", () => {
    expect(parseTaskListQuery("?status=unknown&priority=8&plan=unknown&page=-2&limit=9999&sort=wat&q=%00")).toEqual(defaultTaskListQuery)
  })

  test("builds an absolute same-origin request path through generated path/query parsers", () => {
    const request = buildTaskListRequest("default", {
      ...defaultTaskListQuery,
      status: ["ready"],
      search: "needle",
      page: 2,
      limit: 25,
    })

    expect(request).toBe(
      "/api/v1/boards/default/tasks?status=ready&q=needle&include_archived=false&limit=25&offset=25&sort=updated_at",
    )
  })

  test("builds task map and inspector reads from generated path/query contracts", () => {
    expect(buildTaskMapRequest("default", {
      activeOnly: true,
      contextDepth: 1,
      includeDoneContext: true,
      includeArchivedContext: false,
      hideIsolated: false,
      limitNodes: 240,
    })).toBe("/api/v1/boards/default/task-map?active_only=true&context_depth=1&include_done_context=true&include_archived_context=false&hide_isolated=false&limit_nodes=240")
    expect(buildTaskInspectorRequests("default", "t_1")).toEqual({
      task: "/api/v1/tasks/t_1",
      neighborhood: "/api/v1/tasks/t_1/neighborhood?depth=1&include_archived_context=false&limit_nodes=40",
      dependencies: "/api/v1/tasks/t_1/dependencies",
      steps: "/api/v1/tasks/t_1/steps",
      runs: "/api/v1/tasks/t_1/runs",
      comments: "/api/v1/tasks/t_1/comments",
      events: "/api/v1/events?board=default&task_id=t_1&after=0&limit=50",
    })
  })

  test("surfaces a typed task-not-found error before attempting detail children", async () => {
    const transport = {
      get: async (path: string): Promise<HttpTransportResponse> => {
        if (path.startsWith("/api/v1/boards?")) {
          return {
            payload: {
              data: [{ id: "b_default", slug: "default", name: "Default", description: null, created_at: 1, updated_at: 1, archived_at: null }],
            },
            bytes: 1,
          }
        }
        throw new HttpTransportError("http", "task missing", { status: 404 })
      },
    }

    await expect(loadTaskInspector(runtime, "default", "t_missing", { transport })).rejects.toMatchObject({
      name: "ExplorerReadError",
      kind: "http",
      reason: "task-not-found",
    } satisfies Partial<ExplorerReadError>)
  })
})
