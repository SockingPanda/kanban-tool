import type { ReactNode } from "react"

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
}: {
  readonly title: string
  readonly tasks: readonly InspectorDependency[]
  readonly onSelectTask: (taskId: string) => void
}) {
  return (
    <div className={styles.dependencyGroup}>
      <h3>{title}</h3>
      {tasks.length === 0 ? <Empty>无</Empty> : (
        <ul className={styles.compactList}>
          {tasks.map((task) => (
            <li key={task.id}>
              <button type="button" className={styles.linkButton} onClick={() => onSelectTask(task.id)}>
                <span translate="no">{task.ref}</span>
                <span>{task.title}</span>
                <span className={styles.muted} translate="no">{task.status}</span>
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}

export function TaskInspector({ model, onSelectTask }: TaskInspectorProps) {
  const { task } = model
  return (
    <aside className={styles.inspector} data-testid="task-inspector" aria-label="Task Inspector">
      <header className={styles.header}>
        <p className={styles.eyebrow}>TASK INSPECTOR</p>
        <p className={styles.ref} translate="no">{task.ref}</p>
        <h1>{task.title}</h1>
        <p className={styles.identity} translate="no">{task.id}</p>
        <div className={styles.badges}>
          <span className={styles.badge}>{task.status}</span>
          <span className={styles.badge}>P{task.priority}</span>
          {task.dependencyBlocked ? <span className={styles.badge}>依赖阻塞</span> : null}
        </div>
      </header>

      <Section id="inspector-metadata" title="Metadata">
        <p className={styles.description}>{task.description || "暂无描述。"}</p>
        <Facts facts={[
          ["状态原因", valueOrDash(task.statusReason)],
          ["执行者", valueOrDash(task.assignee)],
          ["执行计划", task.executionPlanState],
          ["必需步骤", `${task.completedRequiredStepCount} / ${task.requiredStepCount}`],
          ["可选步骤", String(task.optionalStepCount)],
          ["创建时间", String(task.createdAt)],
          ["更新时间", String(task.updatedAt)],
        ]} />
        <pre className={styles.codeBlock} translate="no">{jsonValue(task.metadata)}</pre>
      </Section>

      <Section id="inspector-claim" title="Runtime / Claim">
        <Facts facts={[
          ["Claim owner", valueOrDash(task.claimOwner)],
          ["Claim expires", valueOrDash(task.claimExpiresAt)],
          ["Last heartbeat", valueOrDash(task.lastHeartbeatAt)],
          ["Current run", valueOrDash(task.currentRunId)],
          ["Retry", `${task.retryCount} / ${valueOrDash(task.maxRetries)}`],
          ["Blocked parents", String(task.unfinishedParentCount)],
        ]} />
      </Section>

      <Section id="inspector-steps" title="Steps">
        {model.steps.length === 0 ? <Empty>暂无步骤。</Empty> : (
          <ol className={styles.compactList}>
            {model.steps.map((step) => (
              <li key={step.id} className={styles.row}>
                <div>
                  <strong>{step.title}</strong>
                  {step.body ? <p className={styles.muted}>{step.body}</p> : null}
                </div>
                <span className={styles.badge}>{step.status}{step.required ? " · required" : ""}</span>
              </li>
            ))}
          </ol>
        )}
      </Section>

      <Section id="inspector-dependencies" title="Dependencies">
        <DependencyList title="Parents" tasks={model.parents} onSelectTask={onSelectTask} />
        <DependencyList title="Children" tasks={model.children} onSelectTask={onSelectTask} />
      </Section>

      <Section id="inspector-comments" title="Comments">
        {model.comments.length === 0 ? <Empty>暂无评论。</Empty> : (
          <ul className={styles.compactList}>
            {model.comments.map((comment) => (
              <li key={comment.id} className={styles.row}>
                <div><strong>{comment.author}</strong><span className={styles.muted}> · {comment.kind}</span><p>{comment.body}</p></div>
                <time dateTime={String(comment.createdAt)}>{comment.createdAt}</time>
              </li>
            ))}
          </ul>
        )}
      </Section>

      <Section id="inspector-runs" title="Runs">
        {model.runs.length === 0 ? <Empty>暂无运行记录。</Empty> : (
          <ul className={styles.compactList}>
            {model.runs.map((run) => (
              <li key={run.id} className={styles.row}>
                <div><strong translate="no">{run.id}</strong><span className={styles.muted}> · {run.status}</span><p>{run.workerProfile || "manual"} · {run.claimOwner}</p>{run.error ? <p className={styles.error}>{run.error}</p> : null}</div>
                <span className={styles.muted}>{run.hasLog ? "log" : ""}</span>
              </li>
            ))}
          </ul>
        )}
      </Section>

      <Section id="inspector-events" title="Events">
        {model.events.length === 0 ? <Empty>暂无事件。</Empty> : (
          <ol className={styles.compactList}>
            {model.events.map((event) => (
              <li key={event.id} className={styles.row}>
                <div><strong translate="no">{event.kind}</strong><p className={styles.muted}>{event.actor || "system"}</p></div>
                <time dateTime={String(event.createdAt)}>{event.createdAt}</time>
              </li>
            ))}
          </ol>
        )}
      </Section>

      <Section id="inspector-runtime" title="Runtime">
        <Facts facts={[
          ["Actor", model.runtime.actor],
          ["API", model.runtime.apiBaseUrl || "/"],
          ["Server", model.runtime.serverVersion],
          ["Protocol", model.runtime.protocolVersion],
          ["Build", model.runtime.webBuildId],
        ]} />
      </Section>
    </aside>
  )
}
