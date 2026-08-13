import { useState } from "react"
import type { Meta, StoryObj } from "@storybook/react-vite"
import { expect, userEvent, waitFor, within } from "storybook/test"

import {
  NavigationIcon,
  ProductRail,
  ProjectsSidebar,
  ResourceHeader,
  type NavigationLabels,
  type NavigationProject,
  type ProductRailItem,
  type ProjectSurface,
} from "../../ui/navigation"
import { assertCanonicalBoardSlug } from "../../lib/board-slug"
import { asCanonicalBoardId } from "../../lib/sync/contracts"
import navigationStyles from "../../ui/navigation/navigation.module.css"
import styles from "./navigation.stories.module.css"

const DEMO_PROJECTS = [
  Object.freeze({
    id: asCanonicalBoardId("b_kanban_tool"),
    slug: assertCanonicalBoardSlug("kanban-tool"),
    name: "kanban-tool",
    description: "Local-first kanban and durable work queue.",
    archivedAt: null,
  }),
  Object.freeze({
    id: asCanonicalBoardId("b_wiki"),
    slug: assertCanonicalBoardSlug("wiki"),
    name: "Wiki",
    description: "Project knowledge and decision ledger.",
    archivedAt: null,
  }),
  Object.freeze({
    id: asCanonicalBoardId("b_story_workshop"),
    slug: assertCanonicalBoardSlug("story-workshop"),
    name: "Story Workshop",
    description: "Local story asset workbench.",
    archivedAt: 1_754_918_400,
  }),
] as const satisfies readonly NavigationProject[]

const ACTIVE_PROJECTS = DEMO_PROJECTS.filter((project) => project.archivedAt === null)

type StoryLocale = "zh" | "en"
type StoryTheme = "light" | "dark"
type StoryDensity = "compact" | "comfortable"

const STORY_LABELS: Record<StoryLocale, Partial<NavigationLabels>> = {
  zh: {
    productNavigation: "产品导航",
    projects: "项目",
    settings: "设置",
    projectSearch: "搜索项目",
    projectSearchPlaceholder: "搜索项目",
    clearSearch: "清除项目搜索",
    projectSearchEmpty: "没有匹配的项目。",
    projectsEmpty: "没有可用的 canonical 项目。",
    projectLoading: "正在加载项目…",
    projectOffline: "当前离线，显示可用的项目快照。",
    projectError: "项目加载失败，显示可用的项目快照。",
    projectStale: "正在显示缓存的项目列表。",
    projectRecovering: "正在恢复项目列表…",
    retry: "重试",
    close: "关闭",
    openProjectNavigation: "打开项目导航",
    closeProjectNavigation: "关闭项目导航",
    collapse: "收起项目导航",
    expand: "展开项目导航",
    overview: "概览",
    tasks: "任务",
    more: "更多",
    breadcrumb: "面包屑",
    projectPicker: "项目切换器",
  },
  en: {
    productNavigation: "Product navigation",
    projects: "Projects",
    settings: "Settings",
    projectSearch: "Search projects",
    projectSearchPlaceholder: "Search projects",
    clearSearch: "Clear project search",
    projectSearchEmpty: "No projects match this search.",
    projectsEmpty: "No canonical projects are available.",
    projectLoading: "Loading projects…",
    projectOffline: "Offline; showing the available project snapshot.",
    projectError: "Projects failed to load; showing the available snapshot.",
    projectStale: "Showing a cached project list.",
    projectRecovering: "Refreshing projects…",
    retry: "Retry",
    close: "Close",
    openProjectNavigation: "Open project navigation",
    closeProjectNavigation: "Close project navigation",
    collapse: "Collapse project navigation",
    expand: "Expand project navigation",
    overview: "Overview",
    tasks: "Tasks",
    more: "More",
    breadcrumb: "Breadcrumb",
    projectPicker: "Project switcher",
  },
}

const STORY_COPY = {
  zh: {
    projectsTitle: "项目",
    projectsDescription: "当前本地实例可用的 canonical board。",
    searchProjects: "搜索项目",
    compact: "紧凑",
    comfortable: "舒适",
    density: "密度",
    settingsDescription: "设置状态（Storybook 占位）；不会回到项目集合。",
    settingsTitle: "设置",
    overviewDescription: "项目 identity 与 description；不会伪造指标。",
    slug: "Slug",
    archiveState: "归档状态",
    archived: "已归档",
    active: "活动",
    explore: "浏览",
    detailsDeferred: "详情与诊断操作保留在右侧操作组；当前 story 不连接 API。",
    tasksPlaceholder: "Tasks canvas navigation context placeholder",
    taskTitle: "任务",
    more: "更多",
    projectSearchEmpty: "没有匹配的项目。",
    tasksDeferred: "生产任务视图交由 Tasks owner；此 story 仅验证导航上下文。",
    taskViews: "任务视图",
    boardView: "看板",
    listView: "列表（规划中）",
    tableView: "表格（规划中）",
    mapView: "关系图（规划中）",
    taskColumns: { todo: "待办", ready: "就绪", running: "运行中", review: "待审核", done: "已完成", blocked: "已阻塞" },
    taskStatuses: { todo: "待办", ready: "就绪", running: "运行中" },
    filters: "筛选",
    display: "显示",
    searchTasks: "搜索任务",
    allTasks: "全部任务",
    taskDetails: "任务详情",
    closeTaskDetails: "关闭任务详情",
    openDetails: "打开详情",
    canonicalTaskStatus: "canonical task status",
    inspectorStatus: "状态",
    requiredStep: "必需步骤",
    run: "运行",
    emptyColumn: "暂无任务",
    emptySelection: "选择任务以打开详情。",
    diagnosticLabels: { runs: "运行", events: "事件", signals: "Signals", ontology: "本体" },
    fixture: "Storybook fixture · demo-only；无 API、SSE 或 mutation path。",
  },
  en: {
    projectsTitle: "Projects",
    projectsDescription: "Canonical boards available on this local instance.",
    searchProjects: "Search projects",
    compact: "Compact",
    comfortable: "Comfortable",
    density: "Density",
    settingsDescription: "Settings state (Storybook placeholder); it does not navigate back to Projects.",
    settingsTitle: "Settings",
    overviewDescription: "Project identity and description; no invented metrics.",
    slug: "Slug",
    archiveState: "Archive state",
    archived: "Archived",
    active: "Active",
    explore: "Explore",
    detailsDeferred: "Details and diagnostics stay in the single right-side action group; this story has no API.",
    tasksPlaceholder: "Tasks canvas navigation context placeholder",
    taskTitle: "Tasks",
    more: "More",
    projectSearchEmpty: "No projects match this search.",
    tasksDeferred: "Production task views remain deferred to the tasks owner; this story only verifies navigation context.",
    taskViews: "Task views",
    boardView: "Board",
    listView: "List (planned)",
    tableView: "Table (planned)",
    mapView: "Map (planned)",
    taskColumns: { todo: "To do", ready: "Ready", running: "Running", review: "Review", done: "Done", blocked: "Blocked" },
    taskStatuses: { todo: "To do", ready: "Ready", running: "Running" },
    filters: "Filters",
    display: "Display",
    searchTasks: "Search tasks",
    allTasks: "All tasks",
    taskDetails: "Task details",
    closeTaskDetails: "Close task details",
    openDetails: "Open details",
    canonicalTaskStatus: "canonical task status",
    inspectorStatus: "Status",
    requiredStep: "Required step",
    run: "Run",
    emptyColumn: "No tasks",
    emptySelection: "Select a task to open its details.",
    diagnosticLabels: { runs: "Runs", events: "Events", signals: "Signals", ontology: "Ontology" },
    fixture: "Storybook fixture · demo-only; no API, SSE, or mutation path.",
  },
} as const

type StoryViewport = "desktop" | "narrow"

type DemoTask = {
  readonly ref: string
  readonly title: string
  readonly status: "todo" | "ready" | "running"
  readonly metadata?: string
  readonly steps?: string
}

const DEMO_COLUMNS: readonly { name: string; tasks: readonly DemoTask[] }[] = [
  {
    name: "Todo",
    tasks: [
      { ref: "#505", title: "Design system foundations", status: "todo", metadata: "Waiting on dependency" },
      { ref: "#506", title: "Projects shell and overview", status: "todo", metadata: "Waiting on dependency" },
      { ref: "#507", title: "Tasks multi-view workspace", status: "todo", metadata: "Waiting on dependency" },
      { ref: "#508", title: "Web/Desktop acceptance", status: "todo", metadata: "Waiting on dependency" },
    ],
  },
  { name: "Ready", tasks: [{ ref: "#503", title: "Plane-only Web UI", status: "ready", steps: "0/5 steps" }] },
  { name: "Running", tasks: [{ ref: "#504", title: "Storybook component lab", status: "running", steps: "0/1 step" }] },
  { name: "Review", tasks: [] },
  { name: "Done", tasks: [] },
  { name: "Blocked", tasks: [] },
]

const DIAGNOSTICS = [
  { id: "runs", label: "Runs", icon: "activity" as const },
  { id: "events", label: "Events", icon: "activity" as const },
  { id: "signals", label: "Signals", icon: "activity" as const },
  { id: "ontology", label: "Ontology", icon: "activity" as const },
] as const

type NavigationCanvasProps = {
  readonly initialSurface: ProjectSurface
  readonly viewport: StoryViewport
  readonly locale?: StoryLocale
  readonly theme?: StoryTheme
}

function NavigationCanvas({ initialSurface, viewport, locale = "zh", theme = "light" }: NavigationCanvasProps) {
  const [surface, setSurface] = useState<ProjectSurface>(initialSurface)
  const [activeProjectSlug, setActiveProjectSlug] = useState<string | undefined>(initialSurface === "projects" ? undefined : DEMO_PROJECTS[0].slug)
  const [projectQuery, setProjectQuery] = useState("")
  const [density, setDensity] = useState<StoryDensity>("comfortable")
  const [railItem, setRailItem] = useState<ProductRailItem>("projects")
  const [sidebarOpen, setSidebarOpen] = useState(viewport === "desktop")
  const currentProject = DEMO_PROJECTS.find((project) => project.slug === activeProjectSlug) ?? DEMO_PROJECTS[0]
  const labels = STORY_LABELS[locale]
  const copy = STORY_COPY[locale]
  const showingSettings = railItem === "settings"

  const openProject = (project: NavigationProject) => {
    setRailItem("projects")
    setActiveProjectSlug(project.slug)
    setSurface("overview")
    if (viewport === "narrow") setSidebarOpen(false)
  }

  const openSurface = (project: NavigationProject, nextSurface: Exclude<ProjectSurface, "projects">) => {
    setRailItem("projects")
    setActiveProjectSlug(project.slug)
    setSurface(nextSurface)
    if (viewport === "narrow") setSidebarOpen(false)
  }

  const breadcrumbs = showingSettings
    ? [{ label: labels.settings ?? "Settings" }]
    : surface === "projects"
      ? [{ label: labels.projects ?? "Projects" }]
      : [{ label: currentProject.name }, { label: surface === "overview" ? (labels.overview ?? "Overview") : (labels.tasks ?? "Tasks") }]

  return (
    <div
      className={`${navigationStyles.navigationRoot} ${styles.storyFrame}`}
      data-viewport={viewport}
      data-theme={theme}
      data-density={density}
      data-testid={`navigation-story-${initialSurface}-${viewport}`}
    >
      <div className={styles.storyCanvas}>
        <ProductRail
          activeItem={railItem}
          labels={labels}
          onNavigate={(item) => {
            setRailItem(item)
            if (item === "projects") {
              setSurface("projects")
              setSidebarOpen(viewport === "desktop")
            }
          }}
        />
        <ProjectsSidebar
          projects={DEMO_PROJECTS}
          activeProjectSlug={!showingSettings && surface !== "projects" ? currentProject.slug : undefined}
          activeSurface={!showingSettings ? surface : undefined}
          activeSection={!showingSettings && surface !== "projects" ? "project" : "projects"}
          projectQuery={projectQuery}
          onProjectQueryChange={setProjectQuery}
          onProjectSelect={openProject}
          onSurfaceSelect={openSurface}
          onSectionSelect={(section) => {
            if (section === "projects") setSurface("projects")
          }}
          labels={labels}
          drawerId="navigation-projects-sidebar"
          open={viewport === "narrow" ? sidebarOpen : undefined}
          onClose={() => setSidebarOpen(false)}
        />
        <main className={styles.storyMainSurface} aria-label={showingSettings ? labels.settings : "Navigation story surface"}>
          <ResourceHeader
            breadcrumbs={breadcrumbs}
            projectSwitchLabel={!showingSettings && surface !== "projects" ? currentProject.name : undefined}
            projectSwitchAriaLabel={labels.projectPicker}
            onProjectSwitch={() => {
              setRailItem("projects")
              setSurface("projects")
            }}
            onMenuToggle={() => setSidebarOpen((isOpen) => !isOpen)}
            menuOpen={sidebarOpen}
            menuControlsId="navigation-projects-sidebar"
            labels={labels}
            actions={!showingSettings && surface === "overview" ? [{ id: "open-tasks", label: labels.tasks ?? "Tasks", kind: "primary", onSelect: () => setSurface("tasks") }] : []}
          />
          {showingSettings ? <SettingsSurface copy={copy} /> : null}
          {!showingSettings && surface === "projects" ? <ProjectsCollection density={density} copy={copy} onDensityChange={setDensity} onOpenProject={openProject} /> : null}
          {!showingSettings && surface === "overview" ? <ProjectOverview copy={copy} project={currentProject} onOpenTasks={() => setSurface("tasks")} /> : null}
          {!showingSettings && surface === "tasks" ? <TasksWorkspace copy={copy} locale={locale} /> : null}
        </main>
      </div>
    </div>
  )
}

function SettingsSurface({ copy }: { readonly copy: (typeof STORY_COPY)[StoryLocale] }) {
  return (
    <section className={styles.storySurfaceContent} aria-labelledby="settings-state-title" data-testid="story-settings-state">
      <div className={styles.storySurfaceHeading}>
        <div>
          <h1 id="settings-state-title">{copy.settingsTitle}</h1>
          <p>{copy.settingsDescription}</p>
        </div>
      </div>
      <p className={styles.storyFixtureNote}>{copy.fixture}</p>
    </section>
  )
}

function ProjectsCollection({
  density,
  copy,
  onDensityChange,
  onOpenProject,
}: {
  readonly density: StoryDensity
  readonly copy: (typeof STORY_COPY)[StoryLocale]
  readonly onDensityChange: (density: StoryDensity) => void
  readonly onOpenProject: (project: NavigationProject) => void
}) {
  const [query, setQuery] = useState("")
  const filteredProjects = ACTIVE_PROJECTS.filter((project) => `${project.name} ${project.slug} ${project.description}`.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase()))

  return (
    <section className={styles.storySurfaceContent} aria-labelledby="projects-collection-title">
      <div className={styles.storyCollectionToolbar}>
        <div className={styles.storySurfaceHeading}>
          <div>
            <h1 id="projects-collection-title">{copy.projectsTitle}</h1>
            <p>{copy.projectsDescription}</p>
          </div>
        </div>
        <label className={styles.storyCollectionSearch}>
          <NavigationIcon name="search" size={16} />
          <span className={navigationStyles.visuallyHidden}>{copy.searchProjects}</span>
          <input type="search" value={query} onChange={(event) => setQuery(event.currentTarget.value)} placeholder={copy.searchProjects} />
        </label>
        <button
          type="button"
          className={styles.storyDensityControl}
          aria-label={copy.density}
          aria-pressed={density === "compact"}
          data-testid="navigation-density-toggle"
          onClick={() => onDensityChange(density === "compact" ? "comfortable" : "compact")}
        >
          <NavigationIcon name="list" size={16} />
          <span>{density === "compact" ? copy.compact : copy.comfortable}</span>
          <NavigationIcon name="chevron-down" size={14} />
        </button>
      </div>

      {filteredProjects.length > 0 ? (
        <ul className={styles.storyProjectRows}>
          {filteredProjects.map((project) => (
            <li key={project.id}>
              <button type="button" className={styles.storyProjectRow} onClick={() => onOpenProject(project)}>
                <NavigationIcon name="folder" size={31} className={styles.storyProjectRowIcon} />
                <span className={styles.storyProjectRowCopy}>
                  <span className={styles.storyProjectRowName}>{project.name}</span>
                  <span className={styles.storyProjectRowSlug}>{project.slug}</span>
                </span>
                <span className={styles.storyProjectRowDescription}>{project.description}</span>
              </button>
            </li>
          ))}
        </ul>
      ) : (
        <p className={styles.storyEmptyNotice} role="status">{copy.projectSearchEmpty}</p>
      )}
      <p className={styles.storyFixtureNote}>{copy.fixture}</p>
    </section>
  )
}

function ProjectOverview({ project, onOpenTasks, copy }: { readonly project: NavigationProject; readonly onOpenTasks: () => void; readonly copy: (typeof STORY_COPY)[StoryLocale] }) {
  return (
    <section className={styles.storySurfaceContent} aria-labelledby="project-overview-title">
      <div className={styles.storyOverviewLayout}>
        <div className={styles.storyOverviewMain}>
          <h1 id="project-overview-title" className={styles.storyOverviewTitle}>{project.name}</h1>
          <p className={styles.storyOverviewDescription}>{project.description ?? copy.overviewDescription}</p>
          <dl className={styles.storyIdentityRows}>
            <div className={styles.storyIdentityRow}>
              <dt>{copy.slug}</dt>
              <dd><code>{project.slug}</code></dd>
            </div>
            <div className={styles.storyIdentityRow}>
              <dt>{copy.archiveState}</dt>
              <dd>{project.archivedAt !== null ? copy.archived : copy.active}</dd>
            </div>
          </dl>
          <div className={styles.storyExplore}>
            <h2>{copy.explore}</h2>
            <button type="button" className={styles.storyExploreButton} onClick={onOpenTasks}>
              <span className={styles.storyExploreButtonCopy}><NavigationIcon name="list" size={18} /><span>{copy.taskTitle}</span></span>
              <NavigationIcon name="chevron-right" size={17} />
            </button>
          </div>
          <p className={styles.storyFixtureNote}>{copy.detailsDeferred}</p>
        </div>
        <aside className={styles.storyDiagnostics} aria-label={`${copy.more} diagnostics`}>
          <h2>{copy.more}</h2>
          <ul className={styles.storyDiagnosticsList}>
            {DIAGNOSTICS.map((item) => (
              <li key={item.id}>
                <button type="button" className={styles.storyDiagnosticsItem} data-demo-only="true" disabled title={copy.detailsDeferred}>
                  <NavigationIcon name={item.icon} size={18} />
                  <span>{copy.diagnosticLabels[item.id]}</span>
                  <NavigationIcon name="chevron-right" size={15} />
                </button>
              </li>
            ))}
          </ul>
        </aside>
      </div>
    </section>
  )
}

function TasksWorkspace({ copy, locale }: { readonly copy: (typeof STORY_COPY)[StoryLocale]; readonly locale: StoryLocale }) {
  const view = "board" as const
  const [inspectorOpen, setInspectorOpen] = useState(true)
  const selectedTask = DEMO_COLUMNS[2].tasks[0]
  const views: readonly { id: typeof view | "list" | "table" | "map"; label: string; icon: "grid" | "list" | "table" | "map" }[] = [
    { id: "board", label: copy.boardView, icon: "grid" },
    { id: "list", label: copy.listView, icon: "list" },
    { id: "table", label: copy.tableView, icon: "table" },
    { id: "map", label: copy.mapView, icon: "map" },
  ]

  return (
    <section className={styles.storyTaskWorkspace} aria-labelledby="tasks-workspace-title">
      <h1 id="tasks-workspace-title" className={navigationStyles.visuallyHidden}>{copy.tasksPlaceholder}</h1>
      <div className={styles.storyTaskToolbar}>
        <div className={styles.storyTaskToolbarGroup}>
          <div className={styles.storyViewSwitch} role="tablist" aria-label={copy.taskViews}>
            {views.map((item) => (
              <button
                key={item.id}
                type="button"
                className={`${styles.storyViewButton} ${view === item.id ? styles.storyViewButtonActive : ""}`}
                role="tab"
                aria-selected={view === item.id}
                aria-disabled="true"
                disabled
                title={copy.tasksDeferred}
              >
                <NavigationIcon name={item.icon} size={16} />
                <span>{item.label}</span>
              </button>
            ))}
          </div>
        </div>
        <div className={styles.storyTaskToolbarGroup}>
          <button type="button" className={styles.storyToolbarButton} disabled title={copy.tasksDeferred}><NavigationIcon name="list" size={16} />{copy.filters}</button>
          <button type="button" className={styles.storyToolbarButton} disabled title={copy.tasksDeferred}><NavigationIcon name="grid" size={16} />{copy.display}</button>
          <button type="button" className={styles.storyToolbarButton} aria-label={copy.searchTasks} disabled title={copy.tasksDeferred}><NavigationIcon name="search" size={17} /></button>
        </div>
      </div>
      <div className={styles.storyTaskFilterBar}>
        <span className={styles.storyFilterChip}><NavigationIcon name="list" size={15} />{copy.allTasks} <NavigationIcon name="close" size={13} /></span>
        <span className={styles.storyTaskMeta}>{copy.boardView} · {copy.canonicalTaskStatus}</span>
      </div>
      <div className={styles.storyTaskBody}>
        <div className={styles.storyBoardScroller} aria-label={copy.boardView}>
          <div className={styles.storyBoardColumns}>
            {DEMO_COLUMNS.map((column) => (
              <section key={column.name} className={styles.storyBoardColumn} aria-labelledby={`task-column-${column.name.toLocaleLowerCase()}`}>
                <h2 id={`task-column-${column.name.toLocaleLowerCase()}`} className={styles.storyBoardColumnHeader}>
                  <span>{copy.taskColumns[column.name.toLocaleLowerCase() as keyof typeof copy.taskColumns]}</span><span className={styles.storyBoardColumnCount}>{column.tasks.length}</span>
                </h2>
                {column.tasks.length > 0 ? column.tasks.map((task) => (
                  <button
                    key={task.ref}
                    type="button"
                    className={`${styles.storyTaskCard} ${task.ref === selectedTask.ref && inspectorOpen ? styles.storyTaskCardSelected : ""}`}
                    onClick={() => setInspectorOpen(true)}
                  >
                    <span className={styles.storyTaskRef}>{task.ref}</span>
                    <span className={styles.storyTaskTitle}>{task.title}</span>
                    <span className={`${styles.storyTaskBadge} ${task.status === "ready" ? styles.storyTaskBadgeReady : task.status === "running" ? styles.storyTaskBadgeRunning : ""}`}>{copy.taskStatuses[task.status]}</span>
                    {task.metadata ? <span className={styles.storyTaskMeta}>{locale === "zh" && task.metadata === "Waiting on dependency" ? "等待依赖" : task.metadata}</span> : null}
                    {task.steps ? <span className={styles.storyTaskMeta}>{locale === "zh" ? task.steps.replace(" steps", " 步骤").replace(" step", " 步骤") : task.steps}</span> : null}
                  </button>
                )) : <p className={styles.storyTaskMeta}>{copy.emptyColumn}</p>}
              </section>
            ))}
          </div>
        </div>
        {inspectorOpen ? (
          <aside className={styles.storyTaskInspector} aria-label={copy.taskDetails}>
            <div className={styles.storyTaskInspectorHeader}>
              <h2>{selectedTask.ref} {selectedTask.title}</h2>
              <button type="button" className={styles.storyInspectorClose} aria-label={copy.closeTaskDetails} onClick={() => setInspectorOpen(false)}><NavigationIcon name="close" size={19} /></button>
            </div>
            <dl className={styles.storyInspectorRows}>
              <div className={styles.storyInspectorRow}><dt>{copy.inspectorStatus}</dt><dd><span className={`${styles.storyTaskBadge} ${styles.storyTaskBadgeRunning}`}>{copy.taskStatuses.running}</span></dd></div>
              <div className={styles.storyInspectorRow}><dt>{copy.requiredStep}</dt><dd>Build a static Storybook</dd></div>
              <div className={styles.storyInspectorRow}><dt>{copy.run}</dt><dd><code className={styles.storyInspectorRun}>r_01KZRMHQA8</code></dd></div>
            </dl>
            <button type="button" className={styles.storyInspectorButton} disabled title={copy.tasksDeferred}>{copy.openDetails}</button>
          </aside>
        ) : (
          <div className={styles.storyEmptyNotice}>{copy.emptySelection}</div>
        )}
      </div>
      <p className={styles.storyFixtureNote}>{copy.tasksDeferred}</p>
    </section>
  )
}

const meta = {
  title: "Navigation/Plane-only Projects",
  parameters: {
    layout: "fullscreen",
    docs: {
      description: {
        component: "纯 presentational/composable 的 Projects 双层导航。Storybook fixture 明确为 demo-only，不连接 API、SSE 或 mutation。",
      },
    },
  },
} satisfies Meta

export default meta
type Story = StoryObj<typeof meta>

type StoryRenderContext = { readonly globals: Record<string, unknown> }

function storyCanvas(initialSurface: ProjectSurface, viewport: StoryViewport, context: StoryRenderContext) {
  return (
    <NavigationCanvas
      initialSurface={initialSurface}
      viewport={viewport}
      locale={context.globals.locale === "en" ? "en" : "zh"}
      theme={context.globals.theme === "dark" ? "dark" : "light"}
    />
  )
}

async function drawerPlay({ canvasElement }: { readonly canvasElement: HTMLElement }): Promise<void> {
  const canvas = within(canvasElement)
  const cycle = async () => {
    const trigger = canvas.getByTestId("resource-header-menu")
    await userEvent.click(trigger)
    await waitFor(() => expect(canvas.getByTestId("projects-sidebar-close")).toHaveFocus())
    await userEvent.keyboard("{Escape}")
    await waitFor(() => expect(canvas.getByTestId("resource-header-menu")).toHaveFocus())
  }

  // Repeat the real focus/click/Escape cycle to catch StrictMode effect replays.
  await cycle()
  await cycle()
}

export const ProjectsCollectionDesktop: Story = {
  name: "Projects collection · desktop",
  render: (_args, context) => storyCanvas("projects", "desktop", context),
}

export const ProjectsCollectionNarrow: Story = {
  name: "Projects collection · narrow drawer",
  render: (_args, context) => storyCanvas("projects", "narrow", context),
  play: drawerPlay,
}

export const ProjectOverviewDesktop: Story = {
  name: "Project Overview · desktop",
  render: (_args, context) => storyCanvas("overview", "desktop", context),
}

export const ProjectOverviewNarrow: Story = {
  name: "Project Overview · narrow drawer",
  render: (_args, context) => storyCanvas("overview", "narrow", context),
}

export const TasksContextDesktop: Story = {
  name: "Tasks context · desktop side-peek",
  render: (_args, context) => storyCanvas("tasks", "desktop", context),
}

export const TasksContextNarrow: Story = {
  name: "Tasks context · narrow sheet",
  render: (_args, context) => storyCanvas("tasks", "narrow", context),
}
