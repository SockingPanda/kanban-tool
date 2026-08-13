import { useEffect, useRef, useState, type MouseEvent, type ReactNode } from "react"

import { Badge } from "@astryxdesign/core/Badge"
import { Banner } from "@astryxdesign/core/Banner"
import { Button } from "@astryxdesign/core/Button"
import { Heading } from "@astryxdesign/core/Heading"
import { Link } from "@astryxdesign/core/Link"
import { List, ListItem } from "@astryxdesign/core/List"
import { Text } from "@astryxdesign/core/Text"

import { CodeBlock, Grid, PageFrame, SafeHStack, SafeVStack } from "@/ui/astryx"

import { ExplorerReadError, loadTaskRuns, type TaskRunsReadModel } from "../../lib/api/explorer-read-model"
import type { Locale } from "../../lib/preferences"
import type { WebRuntimeConfig } from "../../lib/runtime"
import { usePreferences } from "../../lib/use-preferences"
import { localizedErrorMessage } from "../safe-error"

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
  /** Canonical Tasks route used when the Runs URL has no selected task. */
  readonly tasksHref?: string
  /** Client-side fallback for returning to Tasks without inventing a board-wide Runs route. */
  readonly onBackToTasks?: () => void
}

export interface TaskRunsViewProps {
  readonly runtime: WebRuntimeConfig
  readonly taskId: string | null
  readonly invalidationRevision?: number
  readonly online?: boolean
  readonly tasksHref?: string
  readonly onBackToTasks?: () => void
}

type RunsCopy = {
  readonly title: string
  readonly selectTask: string
  readonly backToTasks: string
  readonly taskScope: string
  readonly loading: string
  readonly empty: string
  readonly error: string
  readonly offline: string
  readonly retry: string
  readonly noLog: string
  readonly logShown: string
  readonly logAvailable: string
  readonly noLogForRun: string
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
    selectTask: "选择任务后查看运行记录。",
    backToTasks: "返回任务",
    taskScope: "任务",
    loading: "正在加载运行记录…",
    empty: "当前任务暂无运行记录。",
    error: "运行记录加载失败",
    offline: "当前离线，无法加载运行记录。",
    retry: "重试",
    noLog: "当前任务没有可用的运行日志。",
    logShown: "日志已显示",
    logAvailable: "有可用日志",
    noLogForRun: "当前运行没有日志",
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
    selectTask: "Select a task to inspect runs.",
    backToTasks: "Back to Tasks",
    taskScope: "Task",
    loading: "Loading runs…",
    empty: "No runs for the selected task.",
    error: "Runs failed to load",
    offline: "You are offline; runs cannot be loaded.",
    retry: "Retry",
    noLog: "No log available for the selected task.",
    logShown: "Log shown",
    logAvailable: "Log available",
    noLogForRun: "This run has no log",
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

type RunStatus = TaskRunsReadModel["runs"][number]["status"]

function statusVariant(status: RunStatus): "neutral" | "info" | "success" | "warning" | "error" {
  switch (status) {
    case "running": return "info"
    case "succeeded": return "success"
    case "failed": return "error"
    case "canceled":
    case "expired": return "warning"
    default: return "neutral"
  }
}

function machineToken(value: string | null | undefined, fallback: string): ReactNode {
  return value ? <code translate="no"><Text type="code">{value}</Text></code> : <Text type="supporting">{fallback}</Text>
}

function taskScope(taskId: string, copy: RunsCopy): ReactNode {
  return (
    <Text type="supporting" data-testid="runs-task-scope">
      {copy.taskScope}: {machineToken(taskId, "—")}
    </Text>
  )
}

type RunLogState = "shown" | "available" | "unavailable"

function runLogState(model: TaskRunsReadModel, run: TaskRunsReadModel["runs"][number]): RunLogState {
  if (!run.has_log) return "unavailable"
  if (run.id === model.selectedRunId && run.id === model.log?.run_id) return "shown"
  return "available"
}

function runLogLabel(state: RunLogState, copy: RunsCopy): string {
  if (state === "shown") return copy.logShown
  if (state === "available") return copy.logAvailable
  return copy.noLogForRun
}

function RunRow({ run, copy, locale, logState }: { readonly run: TaskRunsReadModel["runs"][number]; readonly copy: RunsCopy; readonly locale: Locale; readonly logState: RunLogState }) {
  const logLabel = runLogLabel(logState, copy)
  const isSelected = logState === "shown"
  return (
    <ListItem
      data-testid="run-row"
      data-run-id={run.id}
      data-has-log={run.has_log ? "true" : "false"}
      data-log-state={logState}
      data-selected={isSelected ? "true" : "false"}
      label={machineToken(run.id, "—")}
      endContent={(
        <SafeHStack gap={1} align="center" wrap="wrap">
          <Badge variant={statusVariant(run.status)} label={copy.status[run.status]} />
          <Badge variant={logState === "shown" ? "info" : logState === "available" ? "neutral" : "warning"} label={logLabel} />
        </SafeHStack>
      )}
      description={(
        <Text type="supporting" wordBreak="break-word">
          {copy.worker}: {machineToken(run.worker_profile, copy.manual)} · {copy.owner}: {machineToken(run.claim_owner, "—")} · {copy.started}: {timestamp(run.started_at, locale)} · {copy.finished}: {timestamp(run.finished_at, locale)} · {copy.exit}: {run.exit_code === null ? "—" : String(run.exit_code)} · {logLabel}
          {run.error ? <> · {run.error}</> : null}
        </Text>
      )}
    />
  )
}

function LogPanel({ model, copy }: { readonly model: TaskRunsReadModel; readonly copy: RunsCopy }) {
  if (!model.log) {
    return (
      <SafeVStack as="section" gap={2} padding={4} className="min-w-0" data-testid="runs-no-log" role="status">
        <Text as="p" type="supporting">{copy.noLog}</Text>
      </SafeVStack>
    )
  }
  const logState = model.selectedRunId === model.log.run_id ? "shown" : "available"
  return (
    <SafeVStack as="section" gap={2} padding={3} className="min-w-0" data-testid="runs-log" aria-labelledby="runs-log-heading">
      <SafeHStack as="header" gap={2} align="center" wrap="wrap" data-testid="runs-log-context" data-run-id={model.log.run_id} data-log-state={logState} data-selected={logState === "shown" ? "true" : "false"}>
        <Heading level={3} id="runs-log-heading">{copy.runLog}</Heading>
        {machineToken(model.log.run_id, "—")}
        <Badge variant={logState === "shown" ? "info" : "neutral"} label={logState === "shown" ? copy.logShown : copy.logAvailable} />
        {model.log.truncated ? <Badge variant="warning" label={copy.truncated} /> : null}
      </SafeHStack>
      <CodeBlock
        code={model.log.content || copy.emptyLog}
        language="plaintext"
        isWrapped
        container="section"
        maxHeight="evidence"
        label={copy.runLog}
        hasCopy={false}
        copyLabel={copy.runLog}
        copiedLabel={copy.runLog}
        errorLabel={copy.runLog}
        data-testid="runs-log-content"
      />
    </SafeVStack>
  )
}

type StateBoundaryAction = {
  readonly label: string
  readonly href?: string
  readonly onClick?: () => void
}

function isModifiedClick(event: MouseEvent<HTMLAnchorElement | HTMLButtonElement>): boolean {
  return event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey
}

function StateBoundaryAction({ action }: { readonly action: StateBoundaryAction }) {
  if (action.href !== undefined) {
    return (
      <Link
        href={action.href}
        isStandalone
        onClick={(event) => {
          if (action.onClick === undefined || isModifiedClick(event) || event.defaultPrevented) return
          event.preventDefault()
          action.onClick()
        }}
      >
        {action.label}
      </Link>
    )
  }
  if (action.onClick === undefined) return null
  return <Button label={action.label} variant="secondary" size="sm" onClick={action.onClick} />
}

function StateBoundary({
  testId,
  role,
  title,
  description,
  retry,
  scope,
  action,
}: {
  readonly testId: string
  readonly role: "status" | "alert"
  readonly title?: string
  readonly description: string
  readonly retry?: { readonly label: string; readonly onClick: () => void }
  readonly scope?: ReactNode
  readonly action?: StateBoundaryAction
}) {
  const headingId = `${testId}-heading`
  return (
    <SafeVStack as="section" gap={2} padding={4} data-testid={testId} role={role} aria-labelledby={title ? headingId : undefined}>
      <PageFrame
        frame="content"
        bodyLabel={title ?? description}
        header={title ? <SafeVStack gap={1}><Heading level={2} id={headingId}>{title}</Heading>{scope}</SafeVStack> : undefined}
      >
        <SafeVStack gap={2}>
          {!title && scope ? scope : null}
          <Text as="p" type="supporting">{description}</Text>
          {retry ? <Button label={retry.label} variant="secondary" size="sm" onClick={retry.onClick} /> : null}
          {action ? <StateBoundaryAction action={action} /> : null}
        </SafeVStack>
      </PageFrame>
    </SafeVStack>
  )
}

export function TaskRunsPresentation({ locale, taskId, state, onRetry, tasksHref, onBackToTasks }: TaskRunsPresentationProps) {
  const copy = copies[locale]
  if (!taskId) {
    return (
      <StateBoundary
        testId="runs-no-task"
        role="status"
        description={copy.selectTask}
        action={tasksHref !== undefined || onBackToTasks !== undefined ? { label: copy.backToTasks, href: tasksHref, onClick: onBackToTasks } : undefined}
      />
    )
  }
  const scope = taskScope(taskId, copy)
  const kind = errorKind(state.error instanceof Error ? state.error : null)
  if (state.error && !state.data) {
    const offline = kind === "offline"
    return <StateBoundary testId={offline ? "runs-offline" : "runs-error"} role={offline ? "status" : "alert"} title={offline ? copy.offline : copy.error} description={offline ? copy.offline : localizedErrorMessage(state.error, copy.error, locale)} retry={onRetry ? { label: copy.retry, onClick: onRetry } : undefined} scope={scope} />
  }
  if (state.loading && !state.data) {
    return <StateBoundary testId="runs-loading" role="status" description={copy.loading} scope={scope} />
  }
  if (!state.data || state.data.runs.length === 0) {
    if (kind === "offline") return <StateBoundary testId="runs-offline" role="status" description={copy.offline} retry={onRetry ? { label: copy.retry, onClick: onRetry } : undefined} scope={scope} />
    return <StateBoundary testId="runs-empty" role="status" description={copy.empty} scope={scope} />
  }
  const model = state.data
  return (
    <PageFrame
      frame="content"
      data-testid="runs-ready"
      aria-labelledby="runs-heading"
      bodyLabel={copy.title}
      header={(
        <SafeVStack gap={1}>
          <Heading level={2} id="runs-heading">{copy.title}</Heading>
          {scope}
        </SafeVStack>
      )}
    >
      <SafeVStack gap={4}>
      {state.error ? (
        <Banner
          status={kind === "offline" ? "info" : "error"}
          title={kind === "offline" ? copy.offline : copy.error}
          description={kind === "offline" ? copy.offline : localizedErrorMessage(state.error, copy.error, locale)}
          container="section"
          role={kind === "offline" ? "status" : "alert"}
          endContent={onRetry ? <Button label={copy.retry} variant="ghost" size="sm" onClick={onRetry} /> : undefined}
        />
      ) : null}
      <Grid label={copy.title} columns="single" gap={4} align="start" className="md:grid-cols-2">
        <SafeVStack as="section" gap={0} className="min-w-0 max-h-96 overflow-auto overscroll-contain" aria-labelledby="runs-list-heading" tabIndex={0}>
          <Heading level={3} id="runs-list-heading" className="sr-only">{copy.title}</Heading>
          <List density="compact" hasDividers className="min-w-0">
            {model.runs.map((run) => <RunRow key={run.id} run={run} copy={copy} locale={locale} logState={runLogState(model, run)} />)}
          </List>
        </SafeVStack>
        <LogPanel model={model} copy={copy} />
      </Grid>
      </SafeVStack>
    </PageFrame>
  )
}

function useTaskRunsRead(runtime: WebRuntimeConfig, taskId: string | null, invalidationRevision: number, online: boolean): TaskRunsReadState & { readonly retry: () => void } {
  const loadRef = useRef<((signal: AbortSignal) => Promise<TaskRunsReadModel>) | null>(null)
  loadRef.current = taskId ? (signal) => loadTaskRuns(runtime, taskId, { signal }) : null
  const [generation, setGeneration] = useState(0)
  const [state, setState] = useState<TaskRunsReadState>({ data: null, loading: false, error: null })

  useEffect(() => {
    if (!taskId) {
      setState({ data: null, loading: false, error: null })
      return
    }
    if (!online) {
      setState((current) => ({
        data: current.data?.taskId === taskId ? current.data : null,
        loading: false,
        error: new ExplorerReadError("offline", "当前离线，无法加载运行记录。"),
      }))
      return
    }
    const controller = new AbortController()
    let active = true
    setState((current) => ({ data: current.data?.taskId === taskId ? current.data : null, loading: true, error: null }))
    void loadRef.current?.(controller.signal).then(
      (data) => {
        if (active) setState({ data, loading: false, error: null })
      },
      (error: unknown) => {
        if (active && !(error instanceof Error && error.name === "AbortError")) {
          setState((current) => ({ data: current.data?.taskId === taskId ? current.data : null, loading: false, error: error instanceof Error ? error : new Error(String(error)) }))
        }
      },
    )
    return () => {
      active = false
      controller.abort()
    }
  }, [generation, invalidationRevision, online, taskId])

  return { ...state, retry: () => setGeneration((current) => current + 1) }
}

export function TaskRunsView({ runtime, taskId, invalidationRevision = 0, online = typeof navigator === "undefined" || navigator.onLine, tasksHref, onBackToTasks }: TaskRunsViewProps) {
  const { locale } = usePreferences()
  const state = useTaskRunsRead(runtime, taskId, invalidationRevision, online)
  return <TaskRunsPresentation locale={locale} taskId={taskId} state={state} onRetry={state.retry} tasksHref={tasksHref} onBackToTasks={onBackToTasks} />
}
