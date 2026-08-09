import { useEffect, useState, type ReactNode, type SyntheticEvent } from "react"

import type { Locale } from "../../lib/preferences"
import styles from "./TaskInspector.module.css"

export type InspectorTaskStatus = "triage" | "todo" | "scheduled" | "ready" | "running" | "blocked" | "review" | "done" | "archived"
export type InspectorPlanState = "unplanned" | "planned" | "not_required"

export interface TaskInspectorViewModel {
  readonly task: {
    readonly id: string
    readonly ref: string
    readonly title: string
    readonly status: InspectorTaskStatus
    readonly priority: number
    readonly description: string | null
    readonly statusReason: string | null
    readonly assignee: string | null
    readonly executionPlanState: InspectorPlanState
    readonly dependencyBlocked: boolean
    readonly unfinishedParentCount: number
    readonly requiredStepCount: number
    readonly completedRequiredStepCount: number
    readonly optionalStepCount: number
    readonly metadata: unknown
    readonly claimOwner: string | null
    readonly claimExpiresAt: number | null
    readonly lastHeartbeatAt: number | null
    readonly currentRunId: string | null
    readonly retryCount: number
    readonly maxRetries: number | null
    readonly createdAt: number
    readonly updatedAt: number
  }
  readonly steps: readonly {
    readonly id: string
    readonly title: string
    readonly status: "todo" | "done" | "skipped"
    readonly required: boolean
    readonly body: string | null
  }[]
  readonly parents: readonly InspectorDependency[]
  readonly children: readonly InspectorDependency[]
  readonly comments: readonly {
    readonly id: string
    readonly author: string
    readonly kind: "note" | "decision" | "signal"
    readonly body: string
    readonly createdAt: number
  }[]
  readonly runs: readonly {
    readonly id: string
    readonly status: "running" | "succeeded" | "failed" | "canceled" | "expired"
    readonly workerProfile: string | null
    readonly claimOwner: string
    readonly startedAt: number
    readonly finishedAt: number | null
    readonly exitCode: number | null
    readonly error: string | null
    readonly hasLog: boolean
  }[]
  readonly events: readonly {
    readonly id: number
    readonly kind: string
    readonly actor: string | null
    readonly createdAt: number
  }[]
  readonly neighborhood?: {
    readonly centerTaskId: string
    readonly nodes: readonly { readonly id: string; readonly ref: string; readonly title: string; readonly role: string }[]
    readonly edges: readonly { readonly id: string; readonly sourceTaskId: string; readonly targetTaskId: string; readonly kind: string }[]
  }
  readonly runtime: {
    readonly actor: string
    readonly apiBaseUrl: string
    readonly serverVersion: string
    readonly protocolVersion: string
    readonly webBuildId: string
  }
}

export interface InspectorDependency {
  readonly id: string
  readonly ref: string
  readonly title: string
  readonly status: InspectorTaskStatus
}

export interface TaskInspectorProps {
  readonly model: TaskInspectorViewModel
  readonly onSelectTask: (taskId: string) => void
  readonly locale?: Locale
  readonly onLoadRuns?: () => Promise<TaskInspectorViewModel["runs"]>
  readonly onLoadEvents?: () => Promise<TaskInspectorViewModel["events"]>
  readonly onLoadNeighborhood?: () => Promise<NonNullable<TaskInspectorViewModel["neighborhood"]>>
}

type InspectorCopy = {
  readonly ariaLabel: string
  readonly eyebrow: string
  readonly dependencyBlocked: string
  readonly sections: {
    readonly metadata: string
    readonly claim: string
    readonly steps: string
    readonly dependencies: string
    readonly comments: string
    readonly runs: string
    readonly events: string
    readonly neighborhood: string
    readonly runtime: string
  }
  readonly facts: {
    readonly statusReason: string
    readonly assignee: string
    readonly plan: string
    readonly requiredSteps: string
    readonly optionalSteps: string
    readonly createdAt: string
    readonly updatedAt: string
    readonly claimOwner: string
    readonly claimExpires: string
    readonly heartbeat: string
    readonly currentRun: string
    readonly retry: string
    readonly blockedParents: string
    readonly actor: string
    readonly api: string
    readonly server: string
    readonly protocol: string
    readonly build: string
  }
  readonly parents: string
  readonly children: string
  readonly description: string
  readonly showDescription: string
  readonly noDescription: string
  readonly noItems: string
  readonly noSteps: string
  readonly noComments: string
  readonly noRuns: string
  readonly noEvents: string
  readonly noNeighborhood: string
  readonly neighborhoodNodes: string
  readonly neighborhoodEdges: string
  readonly required: string
  readonly manual: string
  readonly system: string
  readonly log: string
  readonly loading: string
  readonly loadError: string
  readonly status: Readonly<Record<InspectorTaskStatus, string>>
  readonly planState: Readonly<Record<InspectorPlanState, string>>
  readonly stepStatus: Readonly<Record<"todo" | "done" | "skipped", string>>
  readonly runStatus: Readonly<Record<"running" | "succeeded" | "failed" | "canceled" | "expired", string>>
  readonly commentKind: Readonly<Record<"note" | "decision" | "signal", string>>
}

const copies: Record<Locale, InspectorCopy> = {
  zh: {
    ariaLabel: "任务 Inspector",
    eyebrow: "TASK INSPECTOR",
    dependencyBlocked: "依赖阻塞",
    sections: { metadata: "元数据", claim: "运行时 / Claim", steps: "步骤", dependencies: "依赖", comments: "评论", runs: "运行记录", events: "事件", neighborhood: "邻域 / 关系图", runtime: "运行时" },
    facts: { statusReason: "状态原因", assignee: "执行者", plan: "执行计划", requiredSteps: "必需步骤", optionalSteps: "可选步骤", createdAt: "创建时间", updatedAt: "更新时间", claimOwner: "Claim owner", claimExpires: "Claim expires", heartbeat: "Last heartbeat", currentRun: "Current run", retry: "重试", blockedParents: "阻塞父任务", actor: "执行者", api: "API", server: "服务版本", protocol: "协议版本", build: "Web 构建" },
    parents: "父任务",
    children: "子任务",
    description: "描述",
    showDescription: "展开描述",
    noDescription: "暂无描述。",
    noItems: "无",
    noSteps: "暂无步骤。",
    noComments: "暂无评论。",
    noRuns: "暂无运行记录。",
    noEvents: "暂无事件。",
    noNeighborhood: "暂无邻域数据。",
    neighborhoodNodes: "节点",
    neighborhoodEdges: "边",
    required: "必需",
    manual: "手动运行",
    system: "系统",
    log: "日志",
    loading: "正在加载…",
    loadError: "加载失败，请重试。",
    status: { triage: "分诊", todo: "待办", scheduled: "已排期", ready: "就绪", running: "运行中", blocked: "已阻塞", review: "待审核", done: "已完成", archived: "已归档" },
    planState: { unplanned: "未规划", planned: "已规划", not_required: "无需计划" },
    stepStatus: { todo: "待办", done: "已完成", skipped: "已跳过" },
    runStatus: { running: "运行中", succeeded: "成功", failed: "失败", canceled: "已取消", expired: "已过期" },
    commentKind: { note: "备注", decision: "决策", signal: "信号" },
  },
  en: {
    ariaLabel: "Task Inspector",
    eyebrow: "TASK INSPECTOR",
    dependencyBlocked: "Blocked by dependencies",
    sections: { metadata: "Metadata", claim: "Runtime / Claim", steps: "Steps", dependencies: "Dependencies", comments: "Comments", runs: "Runs", events: "Events", neighborhood: "Neighborhood / Map", runtime: "Runtime" },
    facts: { statusReason: "Status reason", assignee: "Assignee", plan: "Execution plan", requiredSteps: "Required steps", optionalSteps: "Optional steps", createdAt: "Created", updatedAt: "Updated", claimOwner: "Claim owner", claimExpires: "Claim expires", heartbeat: "Last heartbeat", currentRun: "Current run", retry: "Retry", blockedParents: "Blocked parents", actor: "Actor", api: "API", server: "Server version", protocol: "Protocol version", build: "Web build" },
    parents: "Parents",
    children: "Children",
    description: "Description",
    showDescription: "Show description",
    noDescription: "No description.",
    noItems: "None",
    noSteps: "No steps.",
    noComments: "No comments.",
    noRuns: "No runs.",
    noEvents: "No events.",
    noNeighborhood: "No neighborhood data.",
    neighborhoodNodes: "Nodes",
    neighborhoodEdges: "Edges",
    required: "required",
    manual: "Manual",
    system: "system",
    log: "log",
    loading: "Loading…",
    loadError: "Failed to load. Try again.",
    status: { triage: "Triage", todo: "To do", scheduled: "Scheduled", ready: "Ready", running: "Running", blocked: "Blocked", review: "Review", done: "Done", archived: "Archived" },
    planState: { unplanned: "Unplanned", planned: "Planned", not_required: "Not required" },
    stepStatus: { todo: "To do", done: "Done", skipped: "Skipped" },
    runStatus: { running: "Running", succeeded: "Succeeded", failed: "Failed", canceled: "Canceled", expired: "Expired" },
    commentKind: { note: "Note", decision: "Decision", signal: "Signal" },
  },
}

function valueOrDash(value: string | number | null): string {
  if (value === null || (typeof value === "string" && value.trim().length === 0)) return "—"
  return String(value)
}

function jsonValue(value: unknown): string {
  try {
    return JSON.stringify(value) ?? "{}"
  } catch {
    return "[unserializable metadata]"
  }
}

function Section({ id, title, children }: { readonly id: string; readonly title: string; readonly children: ReactNode }) {
  return (
    <section className={styles.section} id={id} data-testid={id}>
      <h2>{title}</h2>
      {children}
    </section>
  )
}

function Facts({ facts }: { readonly facts: readonly [string, string][] }) {
  return (
    <dl className={styles.facts}>
      {facts.map(([label, value]) => (
        <div key={label}>
          <dt>{label}</dt>
          <dd translate="no">{value}</dd>
        </div>
      ))}
    </dl>
  )
}

function Empty({ children }: { readonly children: ReactNode }) {
  return <p className={styles.empty} role="status">{children}</p>
}

function DependencyList({
  title,
  tasks,
  onSelectTask,
  copy,
}: {
  readonly title: string
  readonly tasks: readonly InspectorDependency[]
  readonly onSelectTask: (taskId: string) => void
  readonly copy: InspectorCopy
}) {
  return (
    <div className={styles.dependencyGroup}>
      <h3>{title}</h3>
      {tasks.length === 0 ? <Empty>{copy.noItems}</Empty> : (
        <ul className={styles.compactList}>
          {tasks.map((task) => (
            <li key={task.id}>
              <button type="button" className={styles.linkButton} onClick={() => onSelectTask(task.id)}>
                <span translate="no">{task.ref}</span>
                <span>{task.title}</span>
                <span className={styles.muted}>{copy.status[task.status]}</span>
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}

function DescriptionDisclosure({ description, copy }: { readonly description: string | null; readonly copy: InspectorCopy }) {
  const [open, setOpen] = useState(false)
  return (
    <details className={styles.descriptionDisclosure} onToggle={(event: SyntheticEvent<HTMLDetailsElement>) => setOpen(event.currentTarget.open)}>
      <summary>{copy.showDescription}</summary>
      {open ? <p className={styles.description}>{description || copy.noDescription}</p> : null}
    </details>
  )
}

function Neighborhood({ model, copy, onSelectTask }: { readonly model: NonNullable<TaskInspectorViewModel["neighborhood"]>; readonly copy: InspectorCopy; readonly onSelectTask: (taskId: string) => void }) {
  return (
    <div className={styles.neighborhood}>
      <div className={styles.neighborhoodFacts}>
        <span>{copy.neighborhoodNodes}: {model.nodes.length}</span>
        <span>{copy.neighborhoodEdges}: {model.edges.length}</span>
      </div>
      {model.nodes.length === 0 ? <Empty>{copy.noNeighborhood}</Empty> : (
        <ul className={styles.compactList}>
          {model.nodes.map((node) => (
            <li key={node.id}>
              <button type="button" className={styles.linkButton} onClick={() => onSelectTask(node.id)}>
                <span translate="no">{node.ref}</span>
                <span>{node.title}</span>
                <span className={styles.muted}>{node.role}</span>
              </button>
            </li>
          ))}
        </ul>
      )}
      {model.edges.length > 0 ? <ul className={styles.compactList}>{model.edges.map((edge) => <li key={edge.id} className={styles.row}><span translate="no">{edge.sourceTaskId} → {edge.targetTaskId}</span><span className={styles.muted}>{edge.kind}</span></li>)}</ul> : null}
    </div>
  )
}

type InspectorSectionStatus = "idle" | "loading" | "ready" | "error"

export function TaskInspector({ model, onSelectTask, locale = "zh", onLoadRuns, onLoadEvents, onLoadNeighborhood }: TaskInspectorProps) {
  const { task } = model
  const copy = copies[locale]
  const [runs, setRuns] = useState(model.runs)
  const [events, setEvents] = useState(model.events)
  const [neighborhood, setNeighborhood] = useState(model.neighborhood)
  const [runsStatus, setRunsStatus] = useState<InspectorSectionStatus>(model.runs.length > 0 ? "ready" : "idle")
  const [eventsStatus, setEventsStatus] = useState<InspectorSectionStatus>(model.events.length > 0 ? "ready" : "idle")
  const [neighborhoodStatus, setNeighborhoodStatus] = useState<InspectorSectionStatus>(model.neighborhood ? "ready" : "idle")

  useEffect(() => {
    setRuns(model.runs)
    setEvents(model.events)
    setNeighborhood(model.neighborhood)
    setRunsStatus(model.runs.length > 0 ? "ready" : "idle")
    setEventsStatus(model.events.length > 0 ? "ready" : "idle")
    setNeighborhoodStatus(model.neighborhood ? "ready" : "idle")
  }, [model.events, model.neighborhood, model.runs, model.task.id])

  const loadRuns = (event: SyntheticEvent<HTMLDetailsElement>) => {
    if (!event.currentTarget.open || runsStatus !== "idle" || !onLoadRuns) return
    setRunsStatus("loading")
    void onLoadRuns().then((value) => {
      setRuns(value)
      setRunsStatus("ready")
    }, () => setRunsStatus("error"))
  }
  const loadEvents = (event: SyntheticEvent<HTMLDetailsElement>) => {
    if (!event.currentTarget.open || eventsStatus !== "idle" || !onLoadEvents) return
    setEventsStatus("loading")
    void onLoadEvents().then((value) => {
      setEvents(value)
      setEventsStatus("ready")
    }, () => setEventsStatus("error"))
  }
  const loadNeighborhood = (event: SyntheticEvent<HTMLDetailsElement>) => {
    if (!event.currentTarget.open || neighborhoodStatus !== "idle" || !onLoadNeighborhood) return
    setNeighborhoodStatus("loading")
    void onLoadNeighborhood().then((value) => {
      setNeighborhood(value)
      setNeighborhoodStatus("ready")
    }, () => setNeighborhoodStatus("error"))
  }
  return (
    <aside className={styles.inspector} data-testid="task-inspector" aria-label={copy.ariaLabel}>
      <header className={styles.header}>
        <p className={styles.eyebrow}>{copy.eyebrow}</p>
        <p className={styles.ref} translate="no">{task.ref}</p>
        <h2>{task.title}</h2>
        <p className={styles.identity} translate="no">{task.id}</p>
        <div className={styles.badges}>
          <span className={styles.badge}>{copy.status[task.status]}</span>
          <span className={styles.badge}>P{task.priority}</span>
          {task.dependencyBlocked ? <span className={styles.badge}>{copy.dependencyBlocked}</span> : null}
        </div>
      </header>

      <Section id="inspector-metadata" title={copy.sections.metadata}>
        <DescriptionDisclosure description={task.description} copy={copy} />
        <Facts facts={[
          [copy.facts.statusReason, valueOrDash(task.statusReason)],
          [copy.facts.assignee, valueOrDash(task.assignee)],
          [copy.facts.plan, copy.planState[task.executionPlanState]],
          [copy.facts.requiredSteps, `${task.completedRequiredStepCount} / ${task.requiredStepCount}`],
          [copy.facts.optionalSteps, String(task.optionalStepCount)],
          [copy.facts.createdAt, String(task.createdAt)],
          [copy.facts.updatedAt, String(task.updatedAt)],
        ]} />
        <pre className={styles.codeBlock} translate="no">{jsonValue(task.metadata)}</pre>
      </Section>

      <Section id="inspector-claim" title={copy.sections.claim}>
        <Facts facts={[
          [copy.facts.claimOwner, valueOrDash(task.claimOwner)],
          [copy.facts.claimExpires, valueOrDash(task.claimExpiresAt)],
          [copy.facts.heartbeat, valueOrDash(task.lastHeartbeatAt)],
          [copy.facts.currentRun, valueOrDash(task.currentRunId)],
          [copy.facts.retry, `${task.retryCount} / ${valueOrDash(task.maxRetries)}`],
          [copy.facts.blockedParents, String(task.unfinishedParentCount)],
        ]} />
      </Section>

      <Section id="inspector-steps" title={copy.sections.steps}>
        {model.steps.length === 0 ? <Empty>{copy.noSteps}</Empty> : (
          <ol className={styles.compactList}>
            {model.steps.map((step) => (
              <li key={step.id} className={styles.row}>
                <div>
                  <strong>{step.title}</strong>
                  {step.body ? <p className={styles.muted}>{step.body}</p> : null}
                </div>
                <span className={styles.badge}>{copy.stepStatus[step.status]}{step.required ? ` · ${copy.required}` : ""}</span>
              </li>
            ))}
          </ol>
        )}
      </Section>

      <Section id="inspector-dependencies" title={copy.sections.dependencies}>
        <DependencyList title={copy.parents} tasks={model.parents} onSelectTask={onSelectTask} copy={copy} />
        <details>
          <summary>{copy.children}</summary>
          <DependencyList title={copy.children} tasks={model.children} onSelectTask={onSelectTask} copy={copy} />
        </details>
      </Section>

      <Section id="inspector-comments" title={copy.sections.comments}>
        {model.comments.length === 0 ? <Empty>{copy.noComments}</Empty> : (
          <ul className={styles.compactList}>
            {model.comments.map((comment) => (
              <li key={comment.id} className={styles.row}>
                <div><strong>{comment.author}</strong><span className={styles.muted}> · {copy.commentKind[comment.kind]}</span><p>{comment.body}</p></div>
                <time dateTime={String(comment.createdAt)}>{comment.createdAt}</time>
              </li>
            ))}
          </ul>
        )}
      </Section>

      <Section id="inspector-runs" title={copy.sections.runs}>
        <details onToggle={loadRuns}>
          <summary>{copy.sections.runs}</summary>
          {runsStatus === "loading" ? <Empty>{copy.loading}</Empty> : runsStatus === "error" ? <Empty>{copy.loadError}</Empty> : runs.length === 0 ? <Empty>{copy.noRuns}</Empty> : (
          <ul className={styles.compactList}>
            {runs.map((run) => (
              <li key={run.id} className={styles.row}>
                <div><strong translate="no">{run.id}</strong><span className={styles.muted}> · {copy.runStatus[run.status]}</span><p>{run.workerProfile || copy.manual} · {run.claimOwner}</p>{run.error ? <p className={styles.error}>{run.error}</p> : null}</div>
                <span className={styles.muted}>{run.hasLog ? copy.log : ""}</span>
              </li>
            ))}
          </ul>
          )}
        </details>
      </Section>

      <Section id="inspector-events" title={copy.sections.events}>
        <details onToggle={loadEvents}>
          <summary>{copy.sections.events}</summary>
          {eventsStatus === "loading" ? <Empty>{copy.loading}</Empty> : eventsStatus === "error" ? <Empty>{copy.loadError}</Empty> : events.length === 0 ? <Empty>{copy.noEvents}</Empty> : (
          <ol className={styles.compactList}>
            {events.map((event) => (
              <li key={event.id} className={styles.row}>
                <div><strong translate="no">{event.kind}</strong><p className={styles.muted}>{event.actor || copy.system}</p></div>
                <time dateTime={String(event.createdAt)}>{event.createdAt}</time>
              </li>
            ))}
          </ol>
          )}
        </details>
      </Section>

      <Section id="inspector-neighborhood" title={copy.sections.neighborhood}>
        <details onToggle={loadNeighborhood}>
          <summary>{copy.sections.neighborhood}</summary>
          {neighborhoodStatus === "loading" ? <Empty>{copy.loading}</Empty> : neighborhoodStatus === "error" ? <Empty>{copy.loadError}</Empty> : neighborhood ? <Neighborhood model={neighborhood} copy={copy} onSelectTask={onSelectTask} /> : <Empty>{copy.noNeighborhood}</Empty>}
        </details>
      </Section>

      <Section id="inspector-runtime" title={copy.sections.runtime}>
        <Facts facts={[
          [copy.facts.actor, model.runtime.actor],
          [copy.facts.api, model.runtime.apiBaseUrl || "/"],
          [copy.facts.server, model.runtime.serverVersion],
          [copy.facts.protocol, model.runtime.protocolVersion],
          [copy.facts.build, model.runtime.webBuildId],
        ]} />
      </Section>
    </aside>
  )
}
