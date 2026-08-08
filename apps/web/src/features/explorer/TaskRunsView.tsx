import { useEffect, useRef, useState } from "react"

import { ExplorerReadError, loadTaskRuns, type TaskRunsReadModel } from "../../lib/api/explorer-read-model"
import type { Locale } from "../../lib/preferences"
import type { WebRuntimeConfig } from "../../lib/runtime"
import { usePreferences } from "../../lib/use-preferences"
import styles from "./TaskRunsView.module.css"

export type TaskRunsReadState = {
  readonly data: TaskRunsReadModel | null
  readonly loading: boolean
  readonly error: ExplorerReadError | Error | null
}

export interface TaskRunsPresentationProps {
  readonly locale: Locale
  readonly taskId: string | null
  readonly state: TaskRunsReadState
  readonly onRetry?: () => void
}

export interface TaskRunsViewProps {
  readonly runtime: WebRuntimeConfig
  readonly taskId: string | null
}

type RunsCopy = {
  readonly title: string
  readonly kicker: string
  readonly selectTask: string
  readonly loading: string
  readonly empty: string
  readonly error: string
  readonly offline: string
  readonly retry: string
  readonly noLog: string
  readonly runLog: string
  readonly emptyLog: string
  readonly truncated: string
  readonly worker: string
  readonly owner: string
  readonly started: string
  readonly finished: string
  readonly exit: string
  readonly manual: string
  readonly status: Readonly<Record<"running" | "succeeded" | "failed" | "canceled" | "expired", string>>
}

const copies: Record<Locale, RunsCopy> = {
  zh: {
    title: "运行记录",
    kicker: "TASK RUNS",
    selectTask: "选择任务后查看运行记录。",
    loading: "正在加载运行记录…",
    empty: "当前任务暂无运行记录。",
    error: "运行记录加载失败",
    offline: "当前离线，无法加载运行记录。",
    retry: "重试",
    noLog: "当前任务没有可用的运行日志。",
    runLog: "运行日志",
    emptyLog: "（日志为空）",
    truncated: "日志已截断",
    worker: "执行配置",
    owner: "执行者",
    started: "开始时间",
    finished: "结束时间",
    exit: "退出码",
    manual: "手动运行",
    status: { running: "运行中", succeeded: "成功", failed: "失败", canceled: "已取消", expired: "已过期" },
  },
  en: {
    title: "Runs",
    kicker: "TASK RUNS",
    selectTask: "Select a task to inspect runs.",
    loading: "Loading runs…",
    empty: "No runs for the selected task.",
    error: "Runs failed to load",
    offline: "You are offline; runs cannot be loaded.",
    retry: "Retry",
    noLog: "No log available for the selected task.",
    runLog: "Run log",
    emptyLog: "(empty log)",
    truncated: "Log truncated",
    worker: "Worker",
    owner: "Owner",
    started: "Started",
    finished: "Finished",
    exit: "Exit code",
    manual: "Manual",
    status: { running: "Running", succeeded: "Succeeded", failed: "Failed", canceled: "Canceled", expired: "Expired" },
  },
}

function errorKind(error: Error | null): string | null {
  if (!error || !("kind" in error)) return null
  const kind = error.kind
  return typeof kind === "string" ? kind : null
}

function timestamp(value: number | null, locale: Locale): string {
  if (value === null) return "—"
  try {
    return new Intl.DateTimeFormat(locale === "en" ? "en-US" : "zh-CN", { dateStyle: "short", timeStyle: "short" }).format(new Date(value))
  } catch {
    return String(value)
  }
}

function RunRow({ run, copy, locale }: { readonly run: TaskRunsReadModel["runs"][number]; readonly copy: RunsCopy; readonly locale: Locale }) {
  return (
    <li className={styles.runRow} data-testid="run-row">
      <header className={styles.runHeader}>
        <span className={styles.runId} translate="no">{run.id}</span>
        <span className={styles.statusBadge}>{copy.status[run.status]}</span>
      </header>
      <dl className={styles.facts}>
        <div><dt>{copy.worker}</dt><dd>{run.worker_profile ?? copy.manual}</dd></div>
        <div><dt>{copy.owner}</dt><dd>{run.claim_owner}</dd></div>
        <div><dt>{copy.started}</dt><dd>{timestamp(run.started_at, locale)}</dd></div>
        <div><dt>{copy.finished}</dt><dd>{timestamp(run.finished_at, locale)}</dd></div>
        <div><dt>{copy.exit}</dt><dd>{run.exit_code === null ? "—" : String(run.exit_code)}</dd></div>
      </dl>
      {run.error ? <p className={styles.runError}>{run.error}</p> : null}
    </li>
  )
}

function LogPanel({ model, copy }: { readonly model: TaskRunsReadModel; readonly copy: RunsCopy }) {
  if (!model.log) return <section className={styles.noLog} data-testid="runs-no-log" role="status"><p>{copy.noLog}</p></section>
  return (
    <section className={styles.logPanel} data-testid="runs-log" aria-labelledby="runs-log-heading">
      <header className={styles.logHeader}>
        <h2 id="runs-log-heading">{copy.runLog}</h2>
        <span className={styles.runId} translate="no">{model.log.run_id}</span>
        {model.log.truncated ? <span className={styles.logTruncated}>{copy.truncated}</span> : null}
      </header>
      <pre aria-label={copy.runLog}>{model.log.content || copy.emptyLog}</pre>
    </section>
  )
}

export function TaskRunsPresentation({ locale, taskId, state, onRetry }: TaskRunsPresentationProps) {
  const copy = copies[locale]
  if (!taskId) {
    return <section className={styles.state} data-testid="runs-no-task" role="status"><p>{copy.selectTask}</p></section>
  }
  const kind = errorKind(state.error instanceof Error ? state.error : null)
  if (state.error && !state.data) {
    const offline = kind === "offline"
    return (
      <section className={styles.state} data-testid={offline ? "runs-offline" : "runs-error"} role={offline ? "status" : "alert"}>
        <h1>{offline ? copy.offline : copy.error}</h1>
        {!offline ? <p>{state.error.message}</p> : null}
        {onRetry ? <button type="button" onClick={onRetry}>{copy.retry}</button> : null}
      </section>
    )
  }
  if (state.loading && !state.data) {
    return <section className={styles.state} data-testid="runs-loading" role="status"><p>{copy.loading}</p></section>
  }
  if (!state.data || state.data.runs.length === 0) {
    return <section className={styles.state} data-testid="runs-empty" role="status"><p>{copy.empty}</p></section>
  }
  return (
    <section className={styles.runs} data-testid="runs-ready" aria-labelledby="runs-heading">
      <header className={styles.heading}>
        <p className={styles.kicker}>{copy.kicker}</p>
        <h1 id="runs-heading">{copy.title}</h1>
        <p className={styles.taskId} translate="no">{taskId}</p>
      </header>
      {state.error ? <div className={styles.refreshError} role="alert">{state.error.message}</div> : null}
      <div className={styles.columns}>
        <section className={styles.runListPanel} aria-labelledby="runs-list-heading" tabIndex={0}>
          <h2 id="runs-list-heading" className={styles.visuallyHidden}>{copy.title}</h2>
          <ul className={styles.runList}>
            {state.data.runs.map((run) => <RunRow key={run.id} run={run} copy={copy} locale={locale} />)}
          </ul>
        </section>
        <LogPanel model={state.data} copy={copy} />
      </div>
    </section>
  )
}

function useTaskRunsRead(runtime: WebRuntimeConfig, taskId: string | null): TaskRunsReadState & { readonly retry: () => void } {
  const loadRef = useRef<((signal: AbortSignal) => Promise<TaskRunsReadModel>) | null>(null)
  loadRef.current = taskId ? (signal) => loadTaskRuns(runtime, taskId, { signal }) : null
  const [generation, setGeneration] = useState(0)
  const [state, setState] = useState<TaskRunsReadState>({ data: null, loading: false, error: null })

  useEffect(() => {
    if (!taskId) {
      setState({ data: null, loading: false, error: null })
      return
    }
    const controller = new AbortController()
    let active = true
    setState({ data: null, loading: true, error: null })
    void loadRef.current?.(controller.signal).then(
      (data) => {
        if (active) setState({ data, loading: false, error: null })
      },
      (error: unknown) => {
        if (active && !(error instanceof Error && error.name === "AbortError")) {
          setState({ data: null, loading: false, error: error instanceof Error ? error : new Error(String(error)) })
        }
      },
    )
    return () => {
      active = false
      controller.abort()
    }
  }, [generation, taskId])

  return { ...state, retry: () => setGeneration((current) => current + 1) }
}

export function TaskRunsView({ runtime, taskId }: TaskRunsViewProps) {
  const { locale } = usePreferences()
  const state = useTaskRunsRead(runtime, taskId)
  return <TaskRunsPresentation locale={locale} taskId={taskId} state={state} onRetry={state.retry} />
}
