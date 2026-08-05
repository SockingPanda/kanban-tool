import { readdirSync, readFileSync } from "node:fs"
import { fileURLToPath } from "node:url"
import { dirname, resolve } from "node:path"

import { describe, expect, it } from "vitest"

import { primaryViews, sidebarViews } from "@/features/navigation/view-types"

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..")

function source(path: string) {
  return readFileSync(resolve(root, path), "utf8")
}

function taskDetailSource() {
  const directory = resolve(root, "features/task-detail")

  return readdirSync(directory)
    .filter((name) => name.endsWith(".tsx"))
    .sort()
    .map((name) => readFileSync(resolve(directory, name), "utf8"))
    .join("\n")
}

function sourceOrTaskDetail(path: string) {
  return path === "features/task-detail/TaskDetail.tsx" ? taskDetailSource() : source(path)
}

describe("desktop shadcn control convergence", () => {
  it("backs tooltips with the Radix/shadcn trigger/content structure", () => {
    const content = source("components/ui/tooltip.tsx")

    expect(content).toContain('@radix-ui/react-tooltip')
    expect(content).toContain("data-slot=\"tooltip-provider\"")
    expect(content).toContain("data-slot=\"tooltip\"")
    expect(content).toContain("TooltipTrigger")
    expect(content).toContain("TooltipContent")
    expect(content).not.toContain("cloneElement")
  })

  it("mounts the TooltipProvider above List tooltip usage", () => {
    const main = source("main.tsx")
    const list = source("features/list/ListView.tsx")

    expect(main).toContain("TooltipProvider")
    expect(main).toContain("<TooltipProvider>")
    expect(main).toContain("<App />")
    expect(list).toContain("TooltipTrigger")
    expect(list).toContain("TooltipContent")
  })

  it("keeps task cards, table task links, dependency chips, and shell nav off ad hoc buttons", () => {
    const files = [
      "app/AppShell.tsx",
      "features/board/BoardView.tsx",
      "features/list/ListView.tsx",
      "features/ontology/OntologyReviewWorkbench.tsx",
      "features/task-detail/TaskDetail.tsx",
    ]

    for (const file of files) {
      expect(sourceOrTaskDetail(file), file).not.toContain("<button")
    }
  })

  it("keeps scoped toolbar, list, and detail controls off native select inputs", () => {
    const files = [
      "app/AppShell.tsx",
      "features/list/ListView.tsx",
      "features/task-detail/TaskDetail.tsx",
    ]

    for (const file of files) {
      const content = sourceOrTaskDetail(file)
      expect(content, file).not.toContain("NativeSelect")
      expect(content, file).not.toContain("<select")
      expect(content, file).not.toContain('type="checkbox"')
    }
  })

  it("routes compact single-value menus through a shadcn-compatible Select primitive", () => {
    const content = source("components/ui/menu-select.tsx")

    expect(content).toContain("Select")
    expect(content).toContain("SelectTrigger")
    expect(content).toContain("SelectContent")
    expect(content).not.toContain("DropdownMenuRadioGroup")
  })

  it("keeps List filters, rows-per-page, and button pagination controls", () => {
    const content = source("features/list/ListView.tsx")

    expect(content).toContain('ariaLabel={t("List status filter")}')
    expect(content).toContain("Priority")
    expect(content).toContain('ariaLabel={t("Rows per page")}')
    expect(content).toContain("Pagination")
    expect(content).toContain("PaginationContent")
    expect(content).toContain("onFirstPage")
    expect(content).toContain("onPreviousPage")
    expect(content).toContain("onNextPage")
    expect(content).toContain("onLastPage")
  })

  it("routes shell navigation through the shadcn-compatible sidebar primitive", () => {
    const shell = source("app/AppShell.tsx")
    const sidebar = source("components/ui/sidebar.tsx")

    expect(shell).toContain("SidebarProvider")
    expect(shell).toContain("SidebarInset")
    expect(shell).toContain("Sidebar")
    expect(shell).toContain("SidebarMenuItem")
    expect(shell).toContain("SidebarMenuButton")
    expect(shell).not.toContain("function NavItem")
    expect(sidebar).toContain("SidebarProvider")
    expect(sidebar).toContain("data-slot=\"sidebar\"")
    expect(sidebar).toContain("SidebarMenuButton")
  })

  it("backs separators and pagination with shadcn-compatible primitive contracts", () => {
    const separator = source("components/ui/separator.tsx")
    const pagination = source("components/ui/pagination.tsx")

    expect(separator).toContain("@radix-ui/react-separator")
    expect(separator).toContain("SeparatorPrimitive.Root")
    expect(separator).toContain("data-slot=\"separator\"")
    expect(pagination).toContain("PaginationLink")
    expect(pagination).toContain("PaginationPrevious")
    expect(pagination).toContain("PaginationNext")
    expect(pagination).toContain("data-slot=\"pagination-content\"")
  })

  it("uses the shadcn-compatible item primitive for dense system display rows", () => {
    const item = source("components/ui/item.tsx")
    const files = [
      "features/maintenance/MaintenanceView.tsx",
      "features/health/HealthView.tsx",
      "features/settings/SettingsView.tsx",
      "features/runs/RunsView.tsx",
      "features/task-detail/TaskDetail.tsx",
    ]

    expect(item).toContain("data-slot=\"item\"")
    expect(item).toContain("ItemContent")
    expect(item).toContain("ItemActions")
    expect(item).toContain('className={cn("flex min-w-0 items-center gap-2", className)}')
    expect(item).not.toContain("flex shrink-0 items-center gap-2")

    for (const file of files) {
      expect(sourceOrTaskDetail(file), file).toContain("@/components/ui/item")
    }
  })

  it("keeps the archived shell control as a direct two-state button", () => {
    const content = source("app/AppShell.tsx")

    expect(content).toContain('aria-pressed={showArchived}')
    expect(content).toContain("onShowArchivedChange(!showArchived)")
    expect(content).not.toContain("Include archived tasks")
  })

  it("replaces browser-native confirmations and prompts with shadcn dialog primitives", () => {
    const files = [
      "App.tsx",
      "features/task-detail/TaskDetail.tsx",
      "features/maintenance/MaintenanceView.tsx",
    ]

    for (const file of files) {
      const content = sourceOrTaskDetail(file)
      expect(content, file).not.toContain("window.confirm")
      expect(content, file).not.toContain("window.prompt")
    }

    expect(source("components/ui/alert-dialog.tsx")).toContain("@radix-ui/react-alert-dialog")
    expect(source("components/ui/dialog.tsx")).toContain("@radix-ui/react-dialog")
    expect(source("components/ui/tabs.tsx")).toContain("@radix-ui/react-tabs")
    expect(source("App.tsx")).toContain("AlertDialog")
    expect(source("App.tsx")).toContain("Dialog")
    expect(taskDetailSource()).toContain("AlertDialog")
    expect(source("features/maintenance/MaintenanceView.tsx")).toContain("AlertDialog")
  })

  it("routes shell tabs, dialogs, input compositions, empty states, and field groups through shadcn primitives", () => {
    expect(source("components/ui/input-group.tsx")).toContain("InputGroup")
    expect(source("components/ui/empty.tsx")).toContain("Empty")
    expect(source("components/ui/field.tsx")).toContain("Field")
    expect(source("components/ui/dialog.tsx")).toContain("DialogContent")
    expect(source("components/ui/textarea.tsx")).toContain("Textarea")

    const shell = source("app/AppShell.tsx")
    const detail = taskDetailSource()
    const list = source("features/list/ListView.tsx")
    const events = source("features/events/EventsView.tsx")
    const runs = source("features/runs/RunsView.tsx")
    const maintenance = source("features/maintenance/MaintenanceView.tsx")
    const health = source("features/health/HealthView.tsx")

    expect(shell).toContain("Tabs")
    expect(shell).toContain("TabsList")
    expect(shell).toContain("TabsTrigger")
    expect(shell).toContain("DialogContent")
    expect(shell).toContain("Input")
    expect(shell).toContain("Textarea")
    expect(detail).toContain("InputGroup")
    expect(detail).toContain("Field")

    for (const content of [list, events, runs, detail, maintenance, health]) {
      expect(content).toContain("Empty")
    }
  })

  it("keeps graph and low-frequency desktop views behind lazy import boundaries", () => {
    const shell = source("app/AppShell.tsx")
    const detail = source("features/task-detail/TaskDetail.tsx")
    const map = source("features/task-map/BoardTaskMapView.tsx")
    const layout = source("features/task-map/task-graph-layout.ts")

    expect(shell).toContain("lazy(() => import(\"@/features/task-detail/TaskDetail\")")
    expect(shell).toContain("lazy(() => import(\"@/features/task-map/BoardTaskMapView\")")
    expect(shell).toContain("lazy(() => import(\"@/features/maintenance/MaintenanceView\")")
    expect(shell).toContain("lazy(() =>")
    expect(shell).not.toContain("from \"@/features/task-map/BoardTaskMapView\"")
    expect(shell).not.toContain("from \"@/features/task-detail/TaskDetail\"")

    expect(detail).toContain("lazy(() => import(\"@/features/task-map/TaskGraphCanvas\")")
    expect(detail).toContain("<CollapsibleContent>{open ? children : null}</CollapsibleContent>")
    expect(map).toContain("lazy(() => import(\"./TaskGraphCanvas\")")
    expect(map).not.toContain("from \"./TaskGraphCanvas\"")
    expect(layout).toContain("import(\"elkjs/lib/elk.bundled.js\")")
    expect(layout).not.toContain("import ELK")
  })

  it("keeps all operator views in navigation and reachable shell branches", () => {
    const shell = source("app/AppShell.tsx")

    expect(primaryViews).toEqual(["board", "list", "map", "events"])
    expect(sidebarViews).toEqual(["board", "list", "map", "runs", "events", "signals", "ontology", "maintenance", "health", "settings"])
    for (const view of ["map", "signals", "ontology", "maintenance"] as const) {
      expect(sidebarViews).toContain(view)
      expect(shell).toContain(`if (view === \"${view}\")`)
    }
  })

  it("uses the shadcn-compatible textarea composition for task comments", () => {
    const detail = taskDetailSource()

    expect(source("components/ui/input-group.tsx")).toContain("InputGroupTextarea")
    expect(detail).toContain("InputGroupTextarea")
    expect(detail).toContain('name="comment-body"')
    expect(detail).toContain('aria-label={t("Comment body")}')
    expect(detail).toContain('autoComplete="off"')
  })

  it("exposes local shadcn composites for repeated desktop operator patterns", () => {
    const composites = source("components/ui/composites.tsx")

    expect(composites).toContain("PageToolbar")
    expect(composites).toContain("SectionCard")
    expect(composites).toContain("MetricStrip")
    expect(composites).toContain("TaskStatusBadge")
    expect(composites).toContain("PriorityBadge")
    expect(composites).toContain("TaskIdentityLine")
    expect(composites).toContain("@/components/ui/badge")
    expect(composites).toContain("@/components/ui/card")
    expect(composites).toContain("@/components/ui/item")
  })

  it("routes repeated task badges, identity lines, toolbars, metrics, and section cards through composites", () => {
    const board = source("features/board/BoardView.tsx")
    const list = source("features/list/ListView.tsx")
    const detail = taskDetailSource()
    const map = source("features/task-map/BoardTaskMapView.tsx")
    const maintenance = source("features/maintenance/MaintenanceView.tsx")
    const health = source("features/health/HealthView.tsx")
    const events = source("features/events/EventsView.tsx")
    const runs = source("features/runs/RunsView.tsx")
    const ontology = source("features/ontology/OntologyReviewWorkbench.tsx")

    for (const content of [board, list, detail, map]) {
      expect(content).toContain("TaskStatusBadge")
      expect(content).toContain("PriorityBadge")
      expect(content).not.toContain("function badgeVariant")
    }

    for (const content of [list, detail, map, runs]) {
      expect(content).toContain("TaskIdentityLine")
    }

    for (const content of [list, map, events, runs]) {
      expect(content).toContain("PageToolbar")
    }

    for (const content of [maintenance, health]) {
      expect(content).toContain("SectionCard")
      expect(content).toContain("MetricStrip")
    }

    expect(ontology).toContain("PageToolbar")
    expect(ontology).toContain("SectionCard")
    expect(ontology).toContain("MetricStrip")
  })
})
