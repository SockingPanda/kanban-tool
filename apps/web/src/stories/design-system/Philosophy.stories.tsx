import { useState, type ReactNode } from "react"

import type { Meta, StoryObj } from "@storybook/react-vite"

import { Badge } from "@astryxdesign/core/Badge"
import { Button } from "@astryxdesign/core/Button"
import { Icon } from "@astryxdesign/core/Icon"

import type { BoardTaskViewModel } from "../../features/board/types"
import type { TaskInspectorViewModel } from "../../features/explorer/TaskInspector"
import {
  BoardColumns,
  DisplayMenu,
  FilterBar,
  SidePeekFrame,
  TaskCard,
  TaskStateBoundary,
  TaskTable,
  ViewSwitcher,
  type BoardColumnProps,
  type TasksDensity,
  type TasksLocale,
  type TasksView,
} from "../../ui/tasks"
import styles from "./philosophy.module.css"

/**
 * THESIS: 设计系统是一套可执行的界面判断，不是 token 展柜；每条规则都必须有可观察的 live evidence。
 * OWN-WORLD: restrained canvas/surface/layer 阶梯，14px 高密度 UI，单一 operational blue 与语义状态色。
 * STORY: 11 个独立规则从应用根开始，依次解释层级、布局、状态和文本，再在 canonical task 场景里组合验证。
 * FORM: established-world read surface；以“规则—证据—完整场景”组织可复核的设计系统教学。
 * FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, and DESIGN.md
 */

const meta = {
  title: "Design System/Philosophy",
  parameters: {
    layout: "fullscreen",
    docs: {
      description: {
        component:
          "kanban-tool 的 11 条可执行界面规则。每个 story 先给出 Info，再用独立 live evidence 展示 canvas、surface、layer、布局、状态与 canonical task 组合。",
      },
    },
  },
  tags: ["autodocs"],
} satisfies Meta

export default meta
type Story = StoryObj<typeof meta>
type RuleKey =
  | "applicationRoot"
  | "surfaceSiblings"
  | "layerStacking"
  | "surfaceLayerAssociation"
  | "modalException"
  | "cardListPattern"
  | "sidebarLayoutPattern"
  | "stateVariants"
  | "textColorHierarchy"
  | "completeExample"
  | "commonMistakes"

type Tone = "principle" | "warning"

const COPY = {
  zh: {
    info: "Info · 规则说明",
    evidence: "Live evidence · 实时证据",
    principle: "Principle",
    boundary: "Boundary",
    canvas: "Canvas",
    surface: "Surface",
    layer: "Layer",
    popover: "Popover",
    applicationRoot: {
      title: "ApplicationRoot · 应用根只铺一次画布",
      detail: "canvas 属于应用根；页面、导航和工作区都在其上作为 surface 工作，任何子页面都不重新绘制 canvas。",
      evidence: "一张 canvas，两个并列 surface",
    },
    surfaceSiblings: {
      title: "SurfaceSiblings · 同级区域保持同级",
      detail: "导航、工作区和上下文面板承担不同职责，用布局与 hairline 分隔；不要用一层层卡片制造假的层级。",
      evidence: "三块 sibling surface 的职责分工",
    },
    layerStacking: {
      title: "LayerStacking · 深度只服务局部交互",
      detail: "surface 承载区域，layer 承载局部分组，popover 才获得悬浮阴影；静态区域保持安静。",
      evidence: "surface → layer → popover 的深度阶梯",
    },
    surfaceLayerAssociation: {
      title: "SurfaceLayerAssociation · 先定区域，再定局部工具",
      detail: "工具栏属于工作区 surface 的 layer，不是另一个页面；局部 active 状态只改变 layer，不改变事实边界。",
      evidence: "任务工作区中的 toolbar layer 与内容 surface",
    },
    modalException: {
      title: "ModalException · 模态是有理由的例外",
      detail: "只有需要阻断当前上下文的确认或恢复动作才离开普通层级；scrim、dialog 和返回动作必须同时出现。",
      evidence: "点击按钮打开并关闭一个真实 dialog",
    },
    cardListPattern: {
      title: "CardListPattern · 卡片是列表行的可选投影",
      detail: "卡片只展示 canonical task 字段；选中态是可操作的，状态、ref 和步骤证据保持可扫描。",
      evidence: "可选择的 TaskCard 列表",
    },
    sidebarLayoutPattern: {
      title: "SidebarLayoutPattern · 侧栏稳定，工作区可变",
      detail: "稳定导航收窄为 sidebar，主工作区承载 projection；侧栏不抢夺任务事实，也不制造第二套状态机。",
      evidence: "sidebar + BoardColumns 真实组合",
    },
    stateVariants: {
      title: "StateVariants · 状态是正式内容",
      detail: "loading、empty、offline、stale、recovering 和 error 都说明发生了什么；能恢复时，动作必须真的改变 story。",
      evidence: "六种 TaskStateBoundary 变体",
    },
    textColorHierarchy: {
      title: "TextColorHierarchy · 颜色先表达信息权重",
      detail: "primary 负责事实，secondary 负责解释，tertiary 负责上下文，disabled 只表示不可用；mono 只服务精确标识。",
      evidence: "同一任务的文字层级与精确标识",
    },
    completeExample: {
      title: "CompleteExample · 规则在真实工作面汇合",
      detail: "视图、筛选、密度、任务选择、side peek 和恢复反馈共享同一套交互语法；Storybook 只用隔离 fixture，不连接 mutation path。",
      evidence: "可操作的 Tasks 工作区 fixture",
    },
    commonMistakes: {
      title: "CommonMistakes · 用 wrong / fix 对照守住边界",
      detail: "错误示例必须足够具体：指出视觉或交互越界的位置，再给出能回到 canonical 事实的修复。",
      evidence: "三组 wrong / fix 对照",
    },
    rootLabel: "应用根",
    projectNav: "项目导航",
    taskWorkspace: "任务工作区",
    contextPanel: "上下文面板",
    rootNote: "canvas 不在这些 surface 里面重复出现",
    navigation: "导航",
    workspace: "工作区",
    context: "上下文",
    toolbar: "工作区工具栏",
    filter: "状态：运行中",
    grouping: "分组与操作",
    taskList: "任务列表",
    selected: "当前选择",
    cardHint: "点击任意任务，观察 selected 状态如何保持在列表中。",
    sidebarSelected: "当前项目",
    sidebarQueue: "Durable queue",
    sidebarSignals: "Signals",
    sidebarOntology: "Ontology",
    sidebarDetail: "主工作区可以切换 projection，但 sidebar 仍然稳定。",
    openModal: "打开确认",
    closeModal: "关闭确认",
    modalTitle: "离开当前工作区？",
    modalDetail: "这是需要阻断上下文的确认，因此使用 dialog；普通筛选不应升级为模态。",
    modalCancel: "留在这里",
    modalConfirm: "确认离开",
    modalConfirmed: "已确认；dialog 关闭，背景工作区重新可读。",
    stateAttempt: (count: number) => `已触发恢复动作 ${count} 次。`,
    stateAction: "执行恢复",
    textTitle: "Design system philosophy",
    textBody: "先读取任务状态、依赖与执行证据，再决定是否介入。",
    textSupporting: "上次同步于 14:32 · 当前内容仍可读取",
    textDisabled: "不可用：没有 canonical read model",
    textRef: "run_01KZRMHQA8",
    completeTitle: "任务工作区",
    completeDescription: "真实 task 字段组成的隔离 fixture；每个控件都连接到本地 story state。",
    completeSaved: "状态已记录",
    completeSave: "记录观察",
    completeFixture: "Storybook fixture · 不连接 API / SSE / mutation",
    completeMap: "Map projection 暂不可用",
    completeMapDetail: "当前 story 不伪造关系图；生产 Map 使用 canonical typed read model。",
    completeFilterOpen: "筛选面板已请求打开；这里仅展示本地反馈。",
    mistakeNested: "wrong · Surface 套 Surface",
    mistakeNestedDetail: "用三层白色卡片包住一个区域，层级变成装饰。",
    mistakeNestedFix: "fix · 让区域成为 sibling surface，局部工具降为 layer。",
    mistakeColor: "wrong · 只用颜色表达状态",
    mistakeColorDetail: "红、黄、绿没有文字或恢复路径，读者无法在无色环境中判断。",
    mistakeColorFix: "fix · 保留状态文本，并提供可识别的恢复动作。",
    mistakeMetric: "wrong · 用虚构指标填空",
    mistakeMetricDetail: "progress、risk、owner 没有 typed contract，却被放进任务卡片。",
    mistakeMetricFix: "fix · 只展示 canonical 字段；稀疏也要保持真实。",
  },
  en: {
    info: "Info · rule",
    evidence: "Live evidence",
    principle: "Principle",
    boundary: "Boundary",
    canvas: "Canvas",
    surface: "Surface",
    layer: "Layer",
    popover: "Popover",
    applicationRoot: {
      title: "ApplicationRoot · paint the canvas once",
      detail: "The canvas belongs to the application root. Pages, navigation and workspaces are surfaces on top of it; child pages never repaint the canvas.",
      evidence: "One canvas, two sibling surfaces",
    },
    surfaceSiblings: {
      title: "SurfaceSiblings · keep peer regions peer",
      detail: "Navigation, workspace and context own different responsibilities. Layout and hairlines separate them; nested cards do not create fake depth.",
      evidence: "Three sibling surfaces with distinct jobs",
    },
    layerStacking: {
      title: "LayerStacking · depth serves local interaction",
      detail: "Surfaces own regions, layers own local grouping, and popovers alone float with shadow. Static regions stay quiet.",
      evidence: "surface → layer → popover depth ladder",
    },
    surfaceLayerAssociation: {
      title: "SurfaceLayerAssociation · define the region before its tools",
      detail: "A toolbar is a layer of the workspace surface, not another page. Active state changes the layer without changing the fact boundary.",
      evidence: "Toolbar layer attached to a task surface",
    },
    modalException: {
      title: "ModalException · a modal needs a reason",
      detail: "Only a confirmation or recovery action that must block context leaves the normal layer stack. Scrim, dialog and return action travel together.",
      evidence: "A real dialog that opens and closes",
    },
    cardListPattern: {
      title: "CardListPattern · cards are a selectable list projection",
      detail: "Cards expose canonical task fields only. Selection is observable, while ref, status and step evidence remain scannable.",
      evidence: "Selectable TaskCard list",
    },
    sidebarLayoutPattern: {
      title: "SidebarLayoutPattern · stable sidebar, variable workspace",
      detail: "Stable navigation becomes a sidebar while the workspace owns projections. The sidebar never competes with task facts or invents a second state machine.",
      evidence: "Sidebar + real BoardColumns composition",
    },
    stateVariants: {
      title: "StateVariants · states are product content",
      detail: "Loading, empty, offline, stale, recovering and error explain what happened. When recovery is possible, the action really changes the story.",
      evidence: "Six TaskStateBoundary variants",
    },
    textColorHierarchy: {
      title: "TextColorHierarchy · color carries information weight",
      detail: "Primary states facts, secondary explains, tertiary gives context, and disabled marks unavailability. Mono is reserved for exact identifiers.",
      evidence: "Type and color hierarchy for one task",
    },
    completeExample: {
      title: "CompleteExample · rules meet in a real work surface",
      detail: "Views, filters, density, task selection, side peek and recovery feedback share one interaction grammar. Storybook uses isolated fixtures and never connects to mutation.",
      evidence: "Interactive Tasks workspace fixture",
    },
    commonMistakes: {
      title: "CommonMistakes · protect the boundary with wrong / fix",
      detail: "A wrong example names the exact visual or interaction boundary it crosses, then gives a fix that returns to canonical facts.",
      evidence: "Three wrong / fix comparisons",
    },
    rootLabel: "Application root",
    projectNav: "Project navigation",
    taskWorkspace: "Task workspace",
    contextPanel: "Context panel",
    rootNote: "The canvas is not repainted inside these surfaces",
    navigation: "Navigation",
    workspace: "Workspace",
    context: "Context",
    toolbar: "Workspace toolbar",
    filter: "Status: Running",
    grouping: "Grouping and actions",
    taskList: "Task list",
    selected: "Current selection",
    cardHint: "Select any task to see the selected state stay inside the list.",
    sidebarSelected: "Current project",
    sidebarQueue: "Durable queue",
    sidebarSignals: "Signals",
    sidebarOntology: "Ontology",
    sidebarDetail: "The workspace can switch projections while the sidebar stays stable.",
    openModal: "Open confirmation",
    closeModal: "Close confirmation",
    modalTitle: "Leave this workspace?",
    modalDetail: "This blocks context, so it uses a dialog. A normal filter should never become modal.",
    modalCancel: "Stay here",
    modalConfirm: "Confirm leave",
    modalConfirmed: "Confirmed; the dialog closed and the workspace is readable again.",
    stateAttempt: (count: number) => `Recovery action triggered ${count} time${count === 1 ? "" : "s"}.`,
    stateAction: "Recover",
    textTitle: "Design system philosophy",
    textBody: "Read task state, dependencies and execution evidence before deciding whether to intervene.",
    textSupporting: "Last sync 14:32 · Current content remains readable",
    textDisabled: "Unavailable: no canonical read model",
    textRef: "run_01KZRMHQA8",
    completeTitle: "Tasks workspace",
    completeDescription: "An isolated fixture made from real task fields; every control is wired to local story state.",
    completeSaved: "State recorded",
    completeSave: "Record observation",
    completeFixture: "Storybook fixture · no API / SSE / mutation",
    completeMap: "Map projection unavailable",
    completeMapDetail: "This story does not fake a relation graph; production Map uses the canonical typed read model.",
    completeFilterOpen: "Filter panel requested; this is local story feedback only.",
    mistakeNested: "wrong · Surface inside surface",
    mistakeNestedDetail: "Three white cards wrap one region and turn hierarchy into decoration.",
    mistakeNestedFix: "fix · Make regions sibling surfaces and demote local tools to a layer.",
    mistakeColor: "wrong · Color is the only state signal",
    mistakeColorDetail: "Red, yellow and green have no text or recovery path, so color-blind readers cannot decide.",
    mistakeColorFix: "fix · Keep the status text and provide a recognizable recovery action.",
    mistakeMetric: "wrong · Invented metrics fill space",
    mistakeMetricDetail: "Progress, risk and owner have no typed contract but appear on a task card.",
    mistakeMetricFix: "fix · Show canonical fields only; sparse can still be truthful.",
  },
} as const

function localeFor(value: unknown): TasksLocale {
  return value === "en" ? "en" : "zh"
}

function RulePage({
  locale,
  index,
  rule,
  tone = "principle",
  children,
}: {
  readonly locale: TasksLocale
  readonly index: string
  readonly rule: RuleKey
  readonly tone?: Tone
  readonly children: ReactNode
}) {
  const copy = COPY[locale]
  const detail = copy[rule]
  return (
    <main className={styles.root} lang={locale === "zh" ? "zh-CN" : "en"}>
      <div className={styles.page}>
        <header className={styles.ruleHeader}>
          <div className={styles.ruleIndex} aria-hidden="true">{index}</div>
          <div className={styles.ruleInfo}>
            <h1>{detail.title}</h1>
            <div className={styles.ruleMeta} data-tone={tone}>
              <Badge variant={tone === "warning" ? "warning" : "info"} label={tone === "warning" ? copy.boundary : copy.principle} />
              <span>{copy.info}</span>
            </div>
            <p>{detail.detail}</p>
          </div>
        </header>
        <section className={styles.liveSection} aria-labelledby={`evidence-${rule}`}>
          <header className={styles.liveHeader}>
            <h2 id={`evidence-${rule}`}>{detail.evidence}</h2>
            <div className={styles.liveMeta}><span className={styles.liveDot} aria-hidden="true" />{copy.evidence}</div>
          </header>
          <div className={styles.demoSurface}>{children}</div>
        </section>
      </div>
    </main>
  )
}

function taskFixture(overrides: Partial<BoardTaskViewModel> = {}): BoardTaskViewModel {
  return {
    id: "t_512",
    seq: 512,
    ref: "#512",
    title: "Design system philosophy",
    description: null,
    status: "running",
    position: 0,
    scheduledAt: null,
    dueAt: null,
    lastHeartbeatAt: null,
    statusReason: null,
    labels: [],
    lockVersion: 1,
    priority: 1,
    assignee: null,
    readiness: {
      dependencyBlocked: false,
      unfinishedParentCount: 0,
      executionPlanState: "planned",
      requiredStepCount: 3,
      completedRequiredStepCount: 1,
      optionalStepCount: 0,
    },
    ...overrides,
  }
}

const TASKS: readonly BoardTaskViewModel[] = [
  taskFixture({ id: "t_512", seq: 512, ref: "#512", title: "Design system philosophy", status: "running" }),
  taskFixture({ id: "t_513", seq: 513, ref: "#513", title: "Task workspace composition", status: "ready", position: 1, readiness: { ...taskFixture().readiness, completedRequiredStepCount: 0 } }),
  taskFixture({ id: "t_514", seq: 514, ref: "#514", title: "Narrow viewport acceptance", status: "review", position: 2, readiness: { ...taskFixture().readiness, requiredStepCount: 2, completedRequiredStepCount: 2 } }),
]

const COLUMNS: readonly BoardColumnProps[] = [
  { id: "todo", status: "todo", title: "Todo", tasks: [] },
  { id: "ready", status: "ready", title: "Ready", tasks: [TASKS[1]] },
  { id: "running", status: "running", title: "Running", tasks: [TASKS[0]] },
  { id: "review", status: "review", title: "Review", tasks: [TASKS[2]] },
]

const INSPECTOR: TaskInspectorViewModel = {
  task: {
    id: "t_512",
    ref: "#512",
    title: "Design system philosophy",
    status: "running",
    lockVersion: 1,
    scheduledAt: null,
    dueAt: null,
    priority: 1,
    description: null,
    statusReason: null,
    assignee: null,
    executionPlanState: "planned",
    dependencyBlocked: false,
    unfinishedParentCount: 0,
    requiredStepCount: 3,
    completedRequiredStepCount: 1,
    optionalStepCount: 0,
    metadata: null,
    resultSummary: null,
    result: null,
    claimOwner: null,
    claimExpiresAt: null,
    lastHeartbeatAt: null,
    currentRunId: "r_01KZRMHQA8",
    retryCount: 0,
    maxRetries: null,
    createdAt: 0,
    updatedAt: 0,
  },
  steps: [{ id: "step_512", title: "Build a static Storybook", status: "todo", required: true, body: null }],
  parents: [],
  children: [],
  comments: [],
  runs: [],
  events: [],
  runtime: { actor: "storybook", apiBaseUrl: "/", serverVersion: "demo-only", protocolVersion: "demo-only", webBuildId: "demo-only" },
}

function ApplicationRootDemo({ locale }: { readonly locale: TasksLocale }) {
  const copy = COPY[locale]
  return (
    <div className={styles.applicationRootDemo}>
      <div className={styles.canvasStage} data-depth="canvas">
        <span className={styles.annotation}>{copy.canvas}</span>
        <div className={styles.rootSurface} data-depth="surface">
          <div className={styles.rootSurfaceHeader}><strong>{copy.rootLabel}</strong><span>{copy.rootNote}</span></div>
          <div className={styles.rootSurfaceGrid}>
            <div className={styles.rootRegion}><strong>{copy.projectNav}</strong><span>{copy.surface}</span></div>
            <div className={styles.rootRegion}><strong>{copy.taskWorkspace}</strong><span>{copy.surface}</span></div>
          </div>
        </div>
      </div>
      <div className={styles.tokenRail} aria-label={copy.rootLabel}>
        <span><i data-tone="canvas" />{copy.canvas}</span>
        <span><i data-tone="surface" />{copy.surface}</span>
      </div>
    </div>
  )
}

function SurfaceSiblingsDemo({ locale }: { readonly locale: TasksLocale }) {
  const copy = COPY[locale]
  return (
    <div className={styles.siblingsDemo} data-relationship="siblings">
      {[
        [copy.projectNav, copy.navigation],
        [copy.taskWorkspace, copy.workspace],
        [copy.contextPanel, copy.context],
      ].map(([title, label], index) => (
        <section key={title} className={styles.siblingSurface}>
          <span className={styles.eyebrow}>{String(index + 1).padStart(2, "0")} · {copy.surface}</span>
          <h3>{title}</h3>
          <p>{label}</p>
        </section>
      ))}
    </div>
  )
}

function LayerStackingDemo({ locale }: { readonly locale: TasksLocale }) {
  const copy = COPY[locale]
  return (
    <div className={styles.layerDemo}>
      <div className={styles.layerSurface} data-depth="surface">
        <span className={styles.depthLabel}>{copy.surface}</span>
        <div className={styles.layerInner} data-depth="layer">
          <span className={styles.depthLabel}>{copy.layer}</span>
          <div className={styles.popover} data-depth="popover">
            <span className={styles.depthLabel}>{copy.popover}</span>
            <strong>{locale === "en" ? "Local options" : "局部选项"}</strong>
            <small>{locale === "en" ? "Only this interaction floats." : "只有这个交互浮起。"}</small>
          </div>
        </div>
      </div>
      <div className={styles.depthLegend}><span>01 {copy.surface}</span><span>02 {copy.layer}</span><span>03 {copy.popover}</span></div>
    </div>
  )
}

function SurfaceLayerAssociationDemo({ locale }: { readonly locale: TasksLocale }) {
  const copy = COPY[locale]
  const [filterActive, setFilterActive] = useState(true)
  return (
    <div className={styles.associationDemo}>
      <section className={styles.associationSurface}>
        <header className={styles.associationToolbar}>
          <div><span className={styles.eyebrow}>{copy.surface}</span><strong>{copy.taskWorkspace}</strong></div>
          <div className={styles.toolbarLayer} data-depth="layer">
            <span>{copy.layer}</span><button type="button" aria-pressed={filterActive} onClick={() => setFilterActive((active) => !active)}>{filterActive ? copy.filter : locale === "en" ? "All tasks" : "全部任务"}</button>
          </div>
        </header>
        <div className={styles.associationContent}>
          <div className={styles.associationLine}><span>{copy.grouping}</span><span className={styles.muted}>{copy.layer}</span></div>
          <div className={styles.associationRows}><span>#512</span><strong>{TASKS[0].title}</strong><Badge variant="info" label={locale === "en" ? "Running" : "运行中"} /></div>
        </div>
      </section>
    </div>
  )
}

function ModalExceptionDemo({ locale }: { readonly locale: TasksLocale }) {
  const copy = COPY[locale]
  const [open, setOpen] = useState(false)
  const [confirmed, setConfirmed] = useState(false)
  return (
    <div className={styles.modalDemo}>
      <div className={styles.modalBackground} aria-hidden={open}>
        <div><span className={styles.eyebrow}>{copy.surface}</span><strong>{copy.taskWorkspace}</strong></div>
        <Button type="button" variant="secondary" label={copy.openModal} onClick={() => { setConfirmed(false); setOpen(true) }} />
      </div>
      {confirmed ? <p className={styles.actionNotice} role="status">{copy.modalConfirmed}</p> : null}
      {open ? (
        <div className={styles.modalScrim}>
          <div className={styles.modalDialog} role="dialog" aria-modal="true" aria-labelledby="philosophy-modal-title">
            <span className={styles.eyebrow}>{copy.boundary}</span>
            <h3 id="philosophy-modal-title">{copy.modalTitle}</h3>
            <p>{copy.modalDetail}</p>
            <div className={styles.modalActions}>
              <Button type="button" variant="ghost" label={copy.modalCancel} onClick={() => setOpen(false)} />
              <Button type="button" variant="primary" label={copy.modalConfirm} onClick={() => { setOpen(false); setConfirmed(true) }} />
            </div>
            <button type="button" className={styles.modalClose} aria-label={copy.closeModal} onClick={() => setOpen(false)}><Icon icon="close" size="sm" /></button>
          </div>
        </div>
      ) : null}
    </div>
  )
}

function CardListPatternDemo({ locale }: { readonly locale: TasksLocale }) {
  const [selectedId, setSelectedId] = useState<string | null>(TASKS[0].id)
  return (
    <div className={styles.cardListDemo}>
      <div className={styles.cardListHeader}><span className={styles.eyebrow}>{COPY[locale].taskList}</span><Badge variant="info" label={`${TASKS.length}`} /></div>
      <div className={styles.cardList}>
        {TASKS.map((task) => <TaskCard key={task.id} task={task} locale={locale} selected={selectedId === task.id} onSelect={(next) => setSelectedId(next.id)} />)}
      </div>
      <p className={styles.interactionNote} role="status">{selectedId ? `${COPY[locale].selected}: ${TASKS.find((task) => task.id === selectedId)?.ref ?? selectedId}` : COPY[locale].cardHint}</p>
    </div>
  )
}

function SidebarLayoutPatternDemo({ locale }: { readonly locale: TasksLocale }) {
  const copy = COPY[locale]
  const [selectedId, setSelectedId] = useState<string | null>(TASKS[0].id)
  return (
    <div className={styles.sidebarDemo}>
      <aside className={styles.demoSidebar} aria-label={copy.navigation}>
        <span className={styles.sidebarBrand}>kanban-tool</span>
        <div className={styles.sidebarProject}><span className={styles.eyebrow}>{copy.sidebarSelected}</span><strong>{locale === "en" ? "Product system" : "产品系统"}</strong></div>
        <nav className={styles.sidebarNav} aria-label={copy.navigation}>
          <span aria-current="page">{copy.sidebarQueue}</span><span>{copy.sidebarSignals}</span><span>{copy.sidebarOntology}</span>
        </nav>
      </aside>
      <section className={styles.sidebarWorkspace} aria-label={copy.workspace}>
        <header><span className={styles.eyebrow}>{copy.surface}</span><h3>{copy.taskWorkspace}</h3><p>{copy.sidebarDetail}</p></header>
        <BoardColumns columns={COLUMNS} locale={locale} selectedTaskId={selectedId} onSelectTask={(task) => setSelectedId(task.id)} />
      </section>
    </div>
  )
}

function StateVariantsDemo({ locale }: { readonly locale: TasksLocale }) {
  const copy = COPY[locale]
  const [attempts, setAttempts] = useState<Partial<Record<"offline" | "stale" | "recovering" | "error", number>>>({})
  const states = ["loading", "empty", "offline", "stale", "recovering", "error"] as const
  return (
    <div className={styles.stateGrid}>
      {states.map((state) => {
        const count = attempts[state as keyof typeof attempts] ?? 0
        const actionable = state === "offline" || state === "stale" || state === "recovering" || state === "error"
        return (
          <div key={state} className={styles.stateItem}>
            <TaskStateBoundary state={state} locale={locale} actionLabel={actionable ? copy.stateAction : undefined} onAction={actionable ? () => setAttempts((current) => ({ ...current, [state]: (current[state] ?? 0) + 1 })) : undefined}>
              {count > 0 ? <p className={styles.stateActionNotice} role="status">{copy.stateAttempt(count)}</p> : null}
            </TaskStateBoundary>
          </div>
        )
      })}
    </div>
  )
}

function TextColorHierarchyDemo({ locale }: { readonly locale: TasksLocale }) {
  const copy = COPY[locale]
  return (
    <article className={styles.typeDemo}>
      <div className={styles.typeMain}><span className={styles.eyebrow}>{copy.taskWorkspace}</span><h3>{copy.textTitle}</h3><p className={styles.textPrimary}>{copy.textBody}</p></div>
      <div className={styles.typeRows}>
        <p className={styles.textSecondary}>{copy.textSupporting}</p>
        <p className={styles.textTertiary}>{copy.contextPanel} · {copy.completeFixture}</p>
        <p className={styles.textDisabled}>{copy.textDisabled}</p>
        <code>{copy.textRef}</code>
      </div>
    </article>
  )
}

function projectionFor({ view, displayVariant, locale, selectedId, onSelectTask, tasks, columns }: { readonly view: TasksView; readonly displayVariant: "grouped" | "table"; readonly locale: TasksLocale; readonly selectedId: string | null; readonly onSelectTask: (task: BoardTaskViewModel) => void; readonly tasks: readonly BoardTaskViewModel[]; readonly columns: readonly BoardColumnProps[] }) {
  if (view === "board") return <BoardColumns columns={columns} locale={locale} selectedTaskId={selectedId} onSelectTask={onSelectTask} />
  if (view === "list" && displayVariant === "table") return <TaskTable tasks={tasks} locale={locale} selectedTaskId={selectedId} onSelectTask={onSelectTask} caption={locale === "en" ? "Task table projection" : "任务表格投影"} />
  if (view === "list") return <div className={styles.listProjection}>{tasks.map((task) => <button key={task.id} type="button" aria-pressed={selectedId === task.id} onClick={() => onSelectTask(task)}><span translate="no">{task.ref}</span><strong>{task.title}</strong><Badge variant="info" label={task.status} /></button>)}</div>
  return <TaskStateBoundary state="offline" locale={locale} title={COPY[locale].completeMap} detail={COPY[locale].completeMapDetail} />
}

function CompleteExampleDemo({ locale }: { readonly locale: TasksLocale }) {
  const copy = COPY[locale]
  const [view, setView] = useState<TasksView>("board")
  const [displayVariant, setDisplayVariant] = useState<"grouped" | "table">("grouped")
  const [density, setDensity] = useState<TasksDensity>("dense")
  const [selectedId, setSelectedId] = useState<string | null>(TASKS[0].id)
  const [search, setSearch] = useState("")
  const [filters, setFilters] = useState(["running"])
  const [filterOpen, setFilterOpen] = useState(false)
  const [saved, setSaved] = useState(false)
  const [peekOpen, setPeekOpen] = useState(false)
  const filteredTasks = search.trim() === "" ? TASKS : TASKS.filter((task) => task.title.toLowerCase().includes(search.toLowerCase()))
  const visibleColumns = COLUMNS.map((column) => ({ ...column, tasks: column.tasks.filter((task) => filteredTasks.some((candidate) => candidate.id === task.id)) }))
  return (
    <div className={styles.completeDemo}>
      <header className={styles.completeHeader}>
        <div><span className={styles.eyebrow}>kanban-tool / Tasks</span><h3>{copy.completeTitle}</h3><p>{copy.completeDescription}</p></div>
        <div className={styles.completeControls}>
          <ViewSwitcher activeView={view} displayVariant={displayVariant} includeTableDisplay onViewChange={setView} onDisplayChange={setDisplayVariant} locale={locale} />
          <DisplayMenu options={{ density }} onDensityChange={setDensity} locale={locale} />
          <Button type="button" variant="secondary" label={saved ? copy.completeSaved : copy.completeSave} onClick={() => setSaved(true)} />
        </div>
      </header>
      <FilterBar search={search} onSearchChange={setSearch} filters={filters.map((id) => ({ id, label: copy.filter, removable: true }))} onRemoveFilter={(id) => setFilters((current) => current.filter((item) => item !== id))} onClearFilters={() => setFilters([])} onOpenFilters={() => setFilterOpen((current) => !current)} locale={locale} />
      {filterOpen ? <p className={styles.actionNotice} role="status">{copy.completeFilterOpen}</p> : null}
      <div className={`${styles.completeWorkspace} ${peekOpen ? styles.completeWithPeek : ""}`} data-density={density}>
        <div className={styles.completeProjection}>{projectionFor({ view, displayVariant, locale, selectedId, tasks: filteredTasks, columns: visibleColumns, onSelectTask: (task) => { setSelectedId(task.id); setPeekOpen(true) } })}</div>
        {peekOpen && selectedId ? <SidePeekFrame model={{ ...INSPECTOR, task: { ...INSPECTOR.task, id: selectedId, ref: TASKS.find((task) => task.id === selectedId)?.ref ?? INSPECTOR.task.ref, title: TASKS.find((task) => task.id === selectedId)?.title ?? INSPECTOR.task.title } }} locale={locale} mode="side-peek" onClose={() => setPeekOpen(false)} onOpenDetails={() => setPeekOpen(false)} /> : null}
      </div>
      <p className={styles.completeFooter} role="note">{copy.completeFixture}</p>
    </div>
  )
}

function CommonMistakesDemo({ locale }: { readonly locale: TasksLocale }) {
  const copy = COPY[locale]
  const [recovered, setRecovered] = useState(false)
  const mistakes = [
    [copy.mistakeNested, copy.mistakeNestedDetail, copy.mistakeNestedFix, "nested"],
    [copy.mistakeColor, copy.mistakeColorDetail, copy.mistakeColorFix, "color"],
    [copy.mistakeMetric, copy.mistakeMetricDetail, copy.mistakeMetricFix, "metric"],
  ] as const
  return (
    <div className={styles.mistakesDemo}>
      {mistakes.map(([title, detail, fix, key]) => (
        <section key={key} className={styles.mistakeRow}>
          <div className={styles.wrongPanel} data-kind={key}><Badge variant="warning" label={locale === "en" ? "wrong" : "wrong"} /><h3>{title}</h3><p>{detail}</p>{key === "nested" ? <div className={styles.wrongNested}><span /><span /><span /></div> : key === "color" ? <div className={styles.wrongSignals}><i data-state="error" /><i data-state="warning" /><i data-state="success" /></div> : <div className={styles.wrongMetric}>97%</div>}</div>
          <div className={styles.fixPanel}><Badge variant="success" label={locale === "en" ? "fix" : "fix"} /><h3>{fix.split(" · ")[0]}</h3><p>{fix.split(" · ").slice(1).join(" · ")}</p><div className={styles.fixEvidence}>{key === "nested" ? <><span>{copy.surface}</span><span>{copy.layer}</span></> : key === "color" ? <><Badge variant="error" label={locale === "en" ? "Error" : "错误"} /><Button type="button" variant="ghost" label={recovered ? copy.completeSaved : copy.stateAction} onClick={() => setRecovered(true)} /></> : <><span className={styles.canonicalField}>#512</span><span>{TASKS[0].title}</span></>}</div>{key === "color" && recovered ? <p className={styles.actionNotice} role="status">{copy.modalConfirmed}</p> : null}</div>
        </section>
      ))}
    </div>
  )
}

function renderRule(rule: RuleKey, render: (locale: TasksLocale) => ReactNode, tone?: Tone): Story["render"] {
  return (_args, context) => <RulePage locale={localeFor(context.globals.locale)} index={ruleIndex(rule)} rule={rule} tone={tone}>{render(localeFor(context.globals.locale))}</RulePage>
}

function ruleIndex(rule: RuleKey): string {
  const indexes: Record<RuleKey, string> = {
    applicationRoot: "01",
    surfaceSiblings: "02",
    layerStacking: "03",
    surfaceLayerAssociation: "04",
    modalException: "05",
    cardListPattern: "06",
    sidebarLayoutPattern: "07",
    stateVariants: "08",
    textColorHierarchy: "09",
    completeExample: "10",
    commonMistakes: "11",
  }
  return indexes[rule]
}

export const ApplicationRoot: Story = { name: "ApplicationRoot", render: renderRule("applicationRoot", (locale) => <ApplicationRootDemo locale={locale} />) }
export const SurfaceSiblings: Story = { name: "SurfaceSiblings", render: renderRule("surfaceSiblings", (locale) => <SurfaceSiblingsDemo locale={locale} />) }
export const LayerStacking: Story = { name: "LayerStacking", render: renderRule("layerStacking", (locale) => <LayerStackingDemo locale={locale} />) }
export const SurfaceLayerAssociation: Story = { name: "SurfaceLayerAssociation", render: renderRule("surfaceLayerAssociation", (locale) => <SurfaceLayerAssociationDemo locale={locale} />) }
export const ModalException: Story = { name: "ModalException", render: renderRule("modalException", (locale) => <ModalExceptionDemo locale={locale} />) }
export const CardListPattern: Story = { name: "CardListPattern", render: renderRule("cardListPattern", (locale) => <CardListPatternDemo locale={locale} />) }
export const SidebarLayoutPattern: Story = { name: "SidebarLayoutPattern", render: renderRule("sidebarLayoutPattern", (locale) => <SidebarLayoutPatternDemo locale={locale} />) }
export const StateVariants: Story = { name: "StateVariants", render: renderRule("stateVariants", (locale) => <StateVariantsDemo locale={locale} />) }
export const TextColorHierarchy: Story = { name: "TextColorHierarchy", render: renderRule("textColorHierarchy", (locale) => <TextColorHierarchyDemo locale={locale} />) }
export const CompleteExample: Story = { name: "CompleteExample", render: renderRule("completeExample", (locale) => <CompleteExampleDemo locale={locale} />) }
export const CommonMistakes: Story = { name: "CommonMistakes", render: renderRule("commonMistakes", (locale) => <CommonMistakesDemo locale={locale} />, "warning") }
