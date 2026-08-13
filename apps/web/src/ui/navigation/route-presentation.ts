import type { AppRoute } from "../../lib/router"
import { parseTasksUrl } from "../../lib/tasks-url"

export type RoutePresentationSurface =
  | "projects"
  | "overview"
  | "board"
  | "list"
  | "table"
  | "map"
  | "runs"
  | "events"
  | "signals"
  | "ontology"
  | "health"
  | "maintenance"
  | "settings"
  | "not-found"

/** The read/session owner a route presentation is describing. */
export type RouteSessionScope = "none" | "board" | "task" | "host" | "host+board"

export type DiagnosticRouteId = "runs" | "events" | "signals" | "ontology" | "health" | "maintenance"

export type RoutePresentationLabels = {
  readonly projects: string
  readonly settings: string
  readonly overview: string
  readonly tasks: string
  readonly board: string
  readonly list: string
  readonly table: string
  readonly map: string
  readonly runs: string
  readonly events: string
  readonly signals: string
  readonly ontology: string
  readonly health: string
  readonly maintenance: string
  readonly notFound: string
  readonly taskSelectionRequired: string
}

export type RoutePresentationOptions = {
  readonly labels: RoutePresentationLabels
  /** Identity-only project name supplied by the canonical board-list snapshot. */
  readonly projectName?: string
}

export type ProjectSelectorPresentation = {
  readonly visible: boolean
}

export type ProjectNavigationPresentation = {
  readonly section: "projects" | "project"
  readonly activeSurface: "projects" | "overview" | "tasks" | null
  readonly overviewActive: boolean
  readonly tasksActive: boolean
}

export type TaskPresentationContext = {
  readonly status: "selected" | "required"
  readonly taskId: string | null
  /** Visible copy keeps the task-scoped Runs route explicit before an inspector exists. */
  readonly label: string
}

export type DiagnosticsMenuItem = {
  readonly id: DiagnosticRouteId
  readonly label: string
  readonly active: boolean
  readonly sessionScope: Exclude<RouteSessionScope, "none">
}

export type DiagnosticsMenuPresentation = {
  readonly active: boolean
  /** The shell's More affordance is active for every diagnostics destination. */
  readonly moreActive: boolean
  readonly activeItem: DiagnosticRouteId | null
  readonly items: readonly DiagnosticsMenuItem[]
}

export type RoutePresentationDescriptor = {
  readonly title: string
  /** Title for the concrete projection, while `title` stays the resource title for Tasks views. */
  readonly surfaceTitle: string
  readonly surface: RoutePresentationSurface
  readonly sessionScope: RouteSessionScope
  readonly projectSelector: ProjectSelectorPresentation
  readonly projectNavigation: ProjectNavigationPresentation
  readonly diagnosticsMenu: DiagnosticsMenuPresentation
  readonly taskContext: TaskPresentationContext | null
}

const diagnosticRouteIds: readonly DiagnosticRouteId[] = [
  "runs",
  "events",
  "signals",
  "ontology",
  "health",
  "maintenance",
]

function isProjectRoute(route: AppRoute): boolean {
  return route.kind === "board"
    || route.kind === "project-overview"
    || route.kind === "health"
    || route.kind === "maintenance"
}

function boardViewSurface(route: Extract<AppRoute, { kind: "board" }>, labels: RoutePresentationLabels): {
  readonly surface: Extract<RoutePresentationSurface, "board" | "list" | "table" | "map" | "runs" | "events" | "signals" | "ontology">
  readonly title: string
  readonly sessionScope: RouteSessionScope
  readonly diagnosticId: DiagnosticRouteId | null
  readonly taskContext: TaskPresentationContext | null
} {
  const view = route.view ?? "board"
  if (view === "runs") {
    const task = parseTasksUrl(route.query ?? "").task
    const taskId = task.kind === "valid" ? task.value : null
    return {
      surface: "runs",
      title: labels.runs,
      sessionScope: "task",
      diagnosticId: "runs",
      taskContext: taskId === null
        ? { status: "required", taskId: null, label: labels.taskSelectionRequired }
        : { status: "selected", taskId, label: taskId },
    }
  }
  if (view === "events") return { surface: "events", title: labels.events, sessionScope: "board", diagnosticId: "events", taskContext: null }
  if (view === "signals") return { surface: "signals", title: labels.signals, sessionScope: "board", diagnosticId: "signals", taskContext: null }
  if (view === "ontology") return { surface: "ontology", title: labels.ontology, sessionScope: "board", diagnosticId: "ontology", taskContext: null }
  if (view === "map") return { surface: "map", title: labels.map, sessionScope: "board", diagnosticId: null, taskContext: null }
  if (view === "list") {
    const display = new URLSearchParams(route.query ?? "").get("display")
    if (display === "table") return { surface: "table", title: labels.table, sessionScope: "board", diagnosticId: null, taskContext: null }
    return { surface: "list", title: labels.list, sessionScope: "board", diagnosticId: null, taskContext: null }
  }
  return { surface: "board", title: labels.board, sessionScope: "board", diagnosticId: null, taskContext: null }
}

function diagnosticSessionScope(id: DiagnosticRouteId): Exclude<RouteSessionScope, "none"> {
  if (id === "runs") return "task"
  if (id === "health") return "host"
  if (id === "maintenance") return "host+board"
  return "board"
}

function diagnosticLabel(id: DiagnosticRouteId, labels: RoutePresentationLabels): string {
  return labels[id]
}

function diagnosticsMenu(
  activeItem: DiagnosticRouteId | null,
  labels: RoutePresentationLabels,
  hasProject: boolean,
): DiagnosticsMenuPresentation {
  if (!hasProject) return { active: false, moreActive: false, activeItem: null, items: [] }
  const items = diagnosticRouteIds.map((id) => ({
    id,
    label: diagnosticLabel(id, labels),
    active: id === activeItem,
    sessionScope: diagnosticSessionScope(id),
  }))
  return { active: activeItem !== null, moreActive: activeItem !== null, activeItem, items }
}

function projectNavigation(route: AppRoute, activeSurface: ProjectNavigationPresentation["activeSurface"]): ProjectNavigationPresentation {
  const section = isProjectRoute(route) ? "project" : "projects"
  return {
    section,
    activeSurface,
    overviewActive: activeSurface === "overview",
    tasksActive: activeSurface === "tasks",
  }
}

/**
 * Build route-only presentation facts for a future shell consumer.
 *
 * This function intentionally does not resolve a board, inspect a task, or
 * call a read/write client. Query state is only used to identify the Table
 * projection and the optional task selection on Runs.
 */
export function describeRoutePresentation(
  route: AppRoute,
  options: RoutePresentationOptions,
): RoutePresentationDescriptor {
  const { labels, projectName } = options
  if (route.kind === "home") {
    return {
      title: labels.projects,
      surfaceTitle: labels.projects,
      surface: "projects",
      sessionScope: "none",
      projectSelector: { visible: false },
      projectNavigation: projectNavigation(route, "projects"),
      diagnosticsMenu: diagnosticsMenu(null, labels, false),
      taskContext: null,
    }
  }
  if (route.kind === "settings") {
    return {
      title: labels.settings,
      surfaceTitle: labels.settings,
      surface: "settings",
      sessionScope: "host",
      projectSelector: { visible: false },
      projectNavigation: projectNavigation(route, null),
      diagnosticsMenu: diagnosticsMenu(null, labels, false),
      taskContext: null,
    }
  }
  if (route.kind === "project-overview") {
    return {
      title: projectName ?? labels.overview,
      surfaceTitle: labels.overview,
      surface: "overview",
      sessionScope: "none",
      projectSelector: { visible: true },
      projectNavigation: projectNavigation(route, "overview"),
      diagnosticsMenu: diagnosticsMenu(null, labels, true),
      taskContext: null,
    }
  }
  if (route.kind === "board") {
    const presentation = boardViewSurface(route, labels)
    return {
      title: presentation.diagnosticId === null ? labels.tasks : presentation.title,
      surfaceTitle: presentation.title,
      surface: presentation.surface,
      sessionScope: presentation.sessionScope,
      projectSelector: { visible: true },
      projectNavigation: projectNavigation(route, presentation.diagnosticId === null ? "tasks" : null),
      diagnosticsMenu: diagnosticsMenu(presentation.diagnosticId, labels, true),
      taskContext: presentation.taskContext,
    }
  }
  if (route.kind === "health" || route.kind === "maintenance") {
    const diagnosticId = route.kind
    const title = route.kind === "health" ? labels.health : labels.maintenance
    return {
      title,
      surfaceTitle: title,
      surface: route.kind,
      sessionScope: route.kind === "health" ? "host" : "host+board",
      projectSelector: { visible: true },
      projectNavigation: projectNavigation(route, null),
      diagnosticsMenu: diagnosticsMenu(diagnosticId, labels, true),
      taskContext: null,
    }
  }
  return {
    title: labels.notFound,
    surfaceTitle: labels.notFound,
    surface: route.kind === "error" || route.kind === "not-found" ? "not-found" : "settings",
    sessionScope: "none",
    projectSelector: { visible: false },
    projectNavigation: projectNavigation(route, null),
    diagnosticsMenu: diagnosticsMenu(null, labels, false),
    taskContext: null,
  }
}

export const createRoutePresentationDescriptor = describeRoutePresentation
export const routePresentationDescriptor = describeRoutePresentation
