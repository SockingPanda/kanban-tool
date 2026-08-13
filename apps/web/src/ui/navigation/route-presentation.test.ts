import { describe, expect, test } from "vitest"

import { parseAppRoute } from "../../lib/router"
import {
  describeRoutePresentation,
  type RoutePresentationLabels,
} from "./route-presentation"

const labels: RoutePresentationLabels = {
  projects: "Projects",
  settings: "Settings",
  overview: "Overview",
  tasks: "Tasks",
  board: "Board",
  list: "List",
  table: "Table",
  map: "Map",
  runs: "Runs",
  events: "Events",
  signals: "Signals",
  ontology: "Ontology",
  health: "Health",
  maintenance: "Maintenance",
  notFound: "Not found",
  taskSelectionRequired: "Select a task to inspect runs.",
}

function route(path: string) {
  return parseAppRoute(`https://kanban.test${path}`)
}

describe("route presentation descriptor", () => {
  test("keeps the identity-only overview title and out of the board session", () => {
    const descriptor = describeRoutePresentation(route("/app/boards/alpha/overview"), {
      labels,
      projectName: "Alpha",
    })

    expect(descriptor.title).toBe("Alpha")
    expect(descriptor.surface).toBe("overview")
    expect(descriptor.sessionScope).toBe("none")
    expect(descriptor.projectSelector.visible).toBe(true)
    expect(descriptor.projectNavigation).toMatchObject({
      section: "project",
      activeSurface: "overview",
      overviewActive: true,
      tasksActive: false,
    })
    expect(descriptor.diagnosticsMenu.active).toBe(false)
  })

  test.each([
    ["/app/boards/alpha/board", "board"],
    ["/app/boards/alpha/list", "list"],
    ["/app/boards/alpha/list?display=table", "table"],
    ["/app/boards/alpha/map", "map"],
  ] as const)("describes the %s task projection as board-scoped", (path, surface) => {
    const descriptor = describeRoutePresentation(route(path), { labels })

    expect(descriptor.title).toBe("Tasks")
    expect(descriptor.surface).toBe(surface)
    expect(descriptor.sessionScope).toBe("board")
    expect(descriptor.projectNavigation).toMatchObject({
      section: "project",
      activeSurface: "tasks",
      overviewActive: false,
      tasksActive: true,
    })
    expect(descriptor.diagnosticsMenu.active).toBe(false)
  })

  test("keeps Runs task-scoped and states the task-selection context without an inspector", () => {
    const selected = describeRoutePresentation(route("/app/boards/alpha/runs?task=t_42"), { labels })
    expect(selected.title).toBe("Runs")
    expect(selected.surface).toBe("runs")
    expect(selected.sessionScope).toBe("task")
    expect(selected.taskContext).toEqual({ status: "selected", taskId: "t_42", label: "t_42" })

    const unselected = describeRoutePresentation(route("/app/boards/alpha/runs"), { labels })
    expect(unselected.sessionScope).toBe("task")
    expect(unselected.taskContext).toEqual({
      status: "required",
      taskId: null,
      label: "Select a task to inspect runs.",
    })
    expect(unselected.projectNavigation).toMatchObject({
      activeSurface: null,
      overviewActive: false,
      tasksActive: false,
    })
    expect(unselected.diagnosticsMenu).toMatchObject({ active: true, moreActive: true, activeItem: "runs" })
  })

  test.each([
    ["events", "board"],
    ["signals", "board"],
    ["ontology", "board"],
    ["health", "host"],
    ["maintenance", "host+board"],
  ] as const)("keeps the %s diagnostics route in the %s scope", (view, sessionScope) => {
    const path = view === "health" || view === "maintenance"
      ? `/app/boards/alpha/${view}`
      : `/app/boards/alpha/${view}`
    const descriptor = describeRoutePresentation(route(path), { labels })

    expect(descriptor.title).toBe(labels[view])
    expect(descriptor.surface).toBe(view)
    expect(descriptor.sessionScope).toBe(sessionScope)
    expect(descriptor.projectSelector.visible).toBe(true)
    expect(descriptor.projectNavigation).toMatchObject({
      section: "project",
      activeSurface: null,
      overviewActive: false,
      tasksActive: false,
    })
    expect(descriptor.diagnosticsMenu).toMatchObject({ active: true, moreActive: true, activeItem: view })
    expect(descriptor.diagnosticsMenu.items.map((item) => item.id)).toEqual([
      "runs",
      "events",
      "signals",
      "ontology",
      "health",
      "maintenance",
    ])
    expect(descriptor.diagnosticsMenu.items.find((item) => item.id === view)?.active).toBe(true)
  })

  test("does not invent project context for collection or non-project routes", () => {
    const home = describeRoutePresentation(route("/app/"), { labels })
    expect(home).toMatchObject({
      title: "Projects",
      surface: "projects",
      sessionScope: "none",
      projectSelector: { visible: false },
      projectNavigation: { section: "projects", activeSurface: "projects" },
    })

    const settings = describeRoutePresentation(route("/app/settings"), { labels })
    expect(settings).toMatchObject({
      title: "Settings",
      surface: "settings",
      sessionScope: "host",
      projectSelector: { visible: false },
      projectNavigation: { section: "projects", activeSurface: null },
    })
  })
})
