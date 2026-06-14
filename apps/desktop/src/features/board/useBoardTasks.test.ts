import { describe, expect, it } from "vitest"

import { resolveBoardTaskRequest } from "./useBoardTasks"

describe("resolveBoardTaskRequest", () => {
  it("keeps board queries on the full first-page board snapshot", () => {
    const request = resolveBoardTaskRequest({
      mode: "board",
      search: "  blocked parent  ",
      statusFilter: "ready",
      priorityFilters: [0, 2],
      sort: "priority",
      showArchived: true,
      limit: 25,
      offset: 75,
    })

    expect(request).toMatchObject({
      search: "blocked parent",
      statusFilter: "all",
      statuses: [],
      priorityFilters: [],
      sort: "-updated_at",
      limit: 25,
      offset: 0,
    })
  })

  it("keeps list filters, sorting, search, and pagination intact", () => {
    const request = resolveBoardTaskRequest({
      mode: "list",
      search: "  dashboard  ",
      statusFilter: "blocked",
      priorityFilters: [1, 3],
      sort: "priority",
      showArchived: false,
      limit: 50,
      offset: 100,
    })

    expect(request).toMatchObject({
      search: "dashboard",
      statusFilter: "blocked",
      statuses: ["blocked"],
      priorityFilters: [1, 3],
      sort: "priority",
      limit: 50,
      offset: 100,
    })
  })
})
