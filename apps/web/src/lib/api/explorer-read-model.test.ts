import { describe, expect, test } from "vitest"

import {
  buildTaskListRequest,
  defaultTaskListQuery,
  parseTaskListQuery,
  serializeTaskListQuery,
} from "./explorer-read-model"

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
})
