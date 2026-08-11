import { useState } from "react"
import type { Meta, StoryObj } from "@storybook/react-vite"

import {
  NavigationIcon,
  ProductRail,
  ProjectsSidebar,
  ResourceHeader,
  type NavigationProject,
  type ProjectSurface,
} from "../../ui/navigation"
import navigationStyles from "../../ui/navigation/navigation.module.css"
import styles from "./navigation.stories.module.css"

const DEMO_PROJECTS = [
  {
    id: "board-kanban-tool",
    slug: "kanban-tool",
    name: "kanban-tool",
    description: "Local-first kanban and durable work queue.",
  },
  {
    id: "board-wiki",
    slug: "wiki",
    name: "Wiki",
    description: "Project knowledge and decision ledger.",
  },
  {
    id: "board-story-workshop",
    slug: "story-workshop",
    name: "Story Workshop",
    description: "Local story asset workbench.",
  },
] as const satisfies readonly NavigationProject[]

type StoryViewport = "desktop" | "narrow"

type DemoTask = {
  readonly ref: string
  readonly title: string
  readonly status: "Todo" | "Ready" | "Running"
  readonly metadata?: string
  readonly steps?: string
}

const DEMO_COLUMNS: readonly { name: string; tasks: readonly DemoTask[] }[] = [
  {
    name: "Todo",
    tasks: [
      { ref: "#505", title: "Design system foundations", status: "Todo", metadata: "Waiting on dependency" },
      { ref: "#506", title: "Projects shell and overview", status: "Todo", metadata: "Waiting on dependency" },
      { ref: "#507", title: "Tasks multi-view workspace", status: "Todo", metadata: "Waiting on dependency" },
      { ref: "#508", title: "Web/Desktop acceptance", status: "Todo", metadata: "Waiting on dependency" },
    ],
  },
  { name: "Ready", tasks: [{ ref: "#503", title: "Plane-only Web UI", status: "Ready", steps: "0/5 steps" }] },
  { name: "Running", tasks: [{ ref: "#504", title: "Storybook component lab", status: "Running", steps: "0/1 step" }] },
  { name: "Review", tasks: [] },
  { name: "Done", tasks: [] },
  { name: "Blocked", tasks: [] },
]

const DIAGNOSTICS = [
  { id: "runs", label: "Runs", icon: "activity" as const },
  { id: "events", label: "Events", icon: "activity" as const },
  { id: "signals", label: "Signals", icon: "activity" as const },
  { id: "ontology", label: "Ontology", icon: "activity" as const },
]

type NavigationCanvasProps = {
  readonly initialSurface: ProjectSurface
  readonly viewport: StoryViewport
}

function NavigationCanvas({ initialSurface, viewport }: NavigationCanvasProps) {
  const [surface, setSurface] = useState<ProjectSurface>(initialSurface)
  const [activeProjectSlug, setActiveProjectSlug] = useState<string | undefined>(initialSurface === "projects" ? undefined : DEMO_PROJECTS[0].slug)
  const [projectQuery, setProjectQuery] = useState("")
  const [sidebarOpen, setSidebarOpen] = useState(viewport === "desktop")
  const currentProject = DEMO_PROJECTS.find((project) => project.slug === activeProjectSlug) ?? DEMO_PROJECTS[0]

  const openProject = (project: NavigationProject) => {
    setActiveProjectSlug(project.slug)
    setSurface("overview")
    if (viewport === "narrow") setSidebarOpen(false)
  }

  const openSurface = (project: NavigationProject, nextSurface: Exclude<ProjectSurface, "projects">) => {
    setActiveProjectSlug(project.slug)
    setSurface(nextSurface)
    if (viewport === "narrow") setSidebarOpen(false)
  }

  const breadcrumbs = surface === "projects"
    ? [{ label: "Projects" }]
    : [{ label: currentProject.name }, { label: surface === "overview" ? "Overview" : "Tasks" }]

  return (
    <div
      className={`${navigationStyles.navigationRoot} ${styles.storyFrame}`}
      data-viewport={viewport}
      data-theme="dark"
      data-testid={`navigation-story-${initialSurface}-${viewport}`}
    >
      <div className={styles.storyCanvas}>
        <ProductRail activeItem="projects" onNavigate={() => setSurface("projects")} />
        <ProjectsSidebar
          projects={DEMO_PROJECTS}
          activeProjectSlug={surface === "projects" ? undefined : currentProject.slug}
          activeSurface={surface}
          activeSection={surface === "projects" ? "projects" : "project"}
          projectQuery={projectQuery}
          onProjectQueryChange={setProjectQuery}
          onProjectSelect={openProject}
          onSurfaceSelect={openSurface}
          onSectionSelect={(section) => {
            if (section === "projects") setSurface("projects")
            if (section === "home") setSurface("projects")
          }}
          open={viewport === "narrow" ? sidebarOpen : undefined}
          onClose={() => setSidebarOpen(false)}
        />
        <main className={styles.storyMainSurface} aria-label="Navigation story surface">
          <ResourceHeader
            breadcrumbs={breadcrumbs}
            projectSwitchLabel={surface === "projects" ? undefined : currentProject.name}
            projectSwitchAriaLabel="Switch project"
            onProjectSwitch={() => setSurface("projects")}
            onMenuToggle={() => setSidebarOpen(true)}
            menuOpen={sidebarOpen}
            actions={surface === "overview" ? [{ id: "open-tasks", label: "Open Tasks", kind: "primary", onSelect: () => setSurface("tasks") }] : []}
            moreItems={surface === "overview" ? DIAGNOSTICS.map((item) => ({ id: item.id, label: item.label, icon: <NavigationIcon name={item.icon} size={16} /> })) : []}
          />
          {surface === "projects" ? <ProjectsCollection onOpenProject={openProject} /> : null}
          {surface === "overview" ? <ProjectOverview project={currentProject} onOpenTasks={() => setSurface("tasks")} /> : null}
          {surface === "tasks" ? <TasksWorkspace /> : null}
        </main>
      </div>
    </div>
  )
}

function ProjectsCollection({ onOpenProject }: { readonly onOpenProject: (project: NavigationProject) => void }) {
  const [query, setQuery] = useState("")
  const filteredProjects = DEMO_PROJECTS.filter((project) => `${project.name} ${project.slug} ${project.description}`.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase()))

  return (
    <section className={styles.storySurfaceContent} aria-labelledby="projects-collection-title">
      <div className={styles.storyCollectionToolbar}>
        <div className={styles.storySurfaceHeading}>
          <div>
            <h1 id="projects-collection-title">Projects</h1>
            <p>Canonical boards available on this local instance.</p>
          </div>
        </div>
        <label className={styles.storyCollectionSearch}>
          <NavigationIcon name="search" size={16} />
          <span className={navigationStyles.visuallyHidden}>Search projects</span>
          <input type="search" value={query} onChange={(event) => setQuery(event.currentTarget.value)} placeholder="Search projects" />
        </label>
        <button type="button" className={styles.storyDensityControl} aria-label="Change density">
          <NavigationIcon name="list" size={16} />
          <span>Dense</span>
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
        <p className={styles.storyEmptyNotice} role="status">No projects match this search.</p>
      )}
      <p className={styles.storyFixtureNote}>Storybook fixture · demo-only canonical board identities; no API or mutation path.</p>
    </section>
  )
}

function ProjectOverview({ project, onOpenTasks }: { readonly project: NavigationProject; readonly onOpenTasks: () => void }) {
  return (
    <section className={styles.storySurfaceContent} aria-labelledby="project-overview-title">
      <div className={styles.storyOverviewLayout}>
        <div className={styles.storyOverviewMain}>
          <h1 id="project-overview-title" className={styles.storyOverviewTitle}>{project.name}</h1>
          <p className={styles.storyOverviewDescription}>{project.description}</p>
          <dl className={styles.storyIdentityRows}>
            <div className={styles.storyIdentityRow}>
              <dt>Slug</dt>
              <dd><code>{project.slug}</code></dd>
            </div>
            <div className={styles.storyIdentityRow}>
              <dt>Archive state</dt>
              <dd>{project.archived ? "Archived" : "Active"}</dd>
            </div>
          </dl>
          <div className={styles.storyExplore}>
            <h2>Explore</h2>
            <button type="button" className={styles.storyExploreButton} onClick={onOpenTasks}>
              <span className={styles.storyExploreButtonCopy}><NavigationIcon name="list" size={18} /><span>Tasks</span></span>
              <NavigationIcon name="chevron-right" size={17} />
            </button>
          </div>
          <p className={styles.storyFixtureNote}>Storybook fixture · identity-only overview; progress, risk, owners and metrics are intentionally absent.</p>
        </div>
        <aside className={styles.storyDiagnostics} aria-label="More diagnostics">
          <h2>More</h2>
          <ul className={styles.storyDiagnosticsList}>
            {DIAGNOSTICS.map((item) => (
              <li key={item.id}>
                <button type="button" className={styles.storyDiagnosticsItem} data-demo-only="true">
                  <NavigationIcon name={item.icon} size={18} />
                  <span>{item.label}</span>
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

function TasksWorkspace() {
  const [view, setView] = useState<"Board" | "List" | "Table" | "Map">("Board")
  const [inspectorOpen, setInspectorOpen] = useState(true)
  const selectedTask = DEMO_COLUMNS[2].tasks[0]
  const views: readonly { name: typeof view; icon: "grid" | "list" | "table" | "map" }[] = [
    { name: "Board", icon: "grid" },
    { name: "List", icon: "list" },
    { name: "Table", icon: "table" },
    { name: "Map", icon: "map" },
  ]

  return (
    <section className={styles.storyTaskWorkspace} aria-labelledby="tasks-workspace-title">
      <h1 id="tasks-workspace-title" className={navigationStyles.visuallyHidden}>Tasks</h1>
      <div className={styles.storyTaskToolbar}>
        <div className={styles.storyTaskToolbarGroup}>
          <div className={styles.storyViewSwitch} role="tablist" aria-label="Task views">
            {views.map((item) => (
              <button
                key={item.name}
                type="button"
                className={`${styles.storyViewButton} ${view === item.name ? styles.storyViewButtonActive : ""}`}
                role="tab"
                aria-selected={view === item.name}
                onClick={() => setView(item.name)}
              >
                <NavigationIcon name={item.icon} size={16} />
                <span>{item.name}</span>
              </button>
            ))}
          </div>
        </div>
        <div className={styles.storyTaskToolbarGroup}>
          <button type="button" className={styles.storyToolbarButton}><NavigationIcon name="list" size={16} />Filters</button>
          <button type="button" className={styles.storyToolbarButton}><NavigationIcon name="grid" size={16} />Display</button>
          <button type="button" className={styles.storyToolbarButton} aria-label="Search tasks"><NavigationIcon name="search" size={17} /></button>
        </div>
      </div>
      <div className={styles.storyTaskFilterBar}>
        <span className={styles.storyFilterChip}><NavigationIcon name="list" size={15} />All tasks <NavigationIcon name="close" size={13} /></span>
        <span className={styles.storyTaskMeta}>{view} view · canonical task status</span>
      </div>
      <div className={styles.storyTaskBody}>
        <div className={styles.storyBoardScroller} aria-label="Task board region">
          <div className={styles.storyBoardColumns}>
            {DEMO_COLUMNS.map((column) => (
              <section key={column.name} className={styles.storyBoardColumn} aria-labelledby={`task-column-${column.name.toLocaleLowerCase()}`}>
                <h2 id={`task-column-${column.name.toLocaleLowerCase()}`} className={styles.storyBoardColumnHeader}>
                  <span>{column.name}</span><span className={styles.storyBoardColumnCount}>{column.tasks.length}</span>
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
                    <span className={`${styles.storyTaskBadge} ${task.status === "Ready" ? styles.storyTaskBadgeReady : task.status === "Running" ? styles.storyTaskBadgeRunning : ""}`}>{task.status}</span>
                    {task.metadata ? <span className={styles.storyTaskMeta}>{task.metadata}</span> : null}
                    {task.steps ? <span className={styles.storyTaskMeta}>{task.steps}</span> : null}
                  </button>
                )) : <p className={styles.storyTaskMeta}>0 tasks</p>}
              </section>
            ))}
          </div>
        </div>
        {inspectorOpen ? (
          <aside className={styles.storyTaskInspector} aria-label="Task details">
            <div className={styles.storyTaskInspectorHeader}>
              <h2>{selectedTask.ref} {selectedTask.title}</h2>
              <button type="button" className={styles.storyInspectorClose} aria-label="Close task details" onClick={() => setInspectorOpen(false)}><NavigationIcon name="close" size={19} /></button>
            </div>
            <dl className={styles.storyInspectorRows}>
              <div className={styles.storyInspectorRow}><dt>Status</dt><dd><span className={`${styles.storyTaskBadge} ${styles.storyTaskBadgeRunning}`}>Running</span></dd></div>
              <div className={styles.storyInspectorRow}><dt>Required step</dt><dd>Build a static Storybook</dd></div>
              <div className={styles.storyInspectorRow}><dt>Run</dt><dd><code className={styles.storyInspectorRun}>r_01KZRMHQA8</code></dd></div>
            </dl>
            <button type="button" className={styles.storyInspectorButton}>Open details</button>
          </aside>
        ) : (
          <div className={styles.storyEmptyNotice}>Select a task to open its details.</div>
        )}
      </div>
      <p className={styles.storyFixtureNote}>Storybook fixture · task status and run reference are demo-only; no API, SSE or mutation path.</p>
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

export const ProjectsCollectionDesktop: Story = {
  name: "Projects collection · desktop",
  render: () => <NavigationCanvas initialSurface="projects" viewport="desktop" />,
}

export const ProjectsCollectionNarrow: Story = {
  name: "Projects collection · narrow drawer",
  render: () => <NavigationCanvas initialSurface="projects" viewport="narrow" />,
}

export const ProjectOverviewDesktop: Story = {
  name: "Project Overview · desktop",
  render: () => <NavigationCanvas initialSurface="overview" viewport="desktop" />,
}

export const ProjectOverviewNarrow: Story = {
  name: "Project Overview · narrow drawer",
  render: () => <NavigationCanvas initialSurface="overview" viewport="narrow" />,
}

export const TasksContextDesktop: Story = {
  name: "Tasks context · desktop side-peek",
  render: () => <NavigationCanvas initialSurface="tasks" viewport="desktop" />,
}

export const TasksContextNarrow: Story = {
  name: "Tasks context · narrow sheet",
  render: () => <NavigationCanvas initialSurface="tasks" viewport="narrow" />,
}
