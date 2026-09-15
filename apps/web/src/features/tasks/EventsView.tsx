import { useAsyncRead } from '../../application/query/use-async-read';
import './activity.css';
import { PageHeader } from '../../components/layout/page-header';
import { Button } from '../../components/ui/button';
import { Icon } from '../../components/ui/icon';
import { Badge } from '../../components/ui/badge';
import { Tabs } from '../../components/ui/tabs';
import { useWorkspaceOperations } from "../../application/workspace/use-workspace-operations";
import { useEffect, useState, type ReactNode } from "react"

import { ExplorerReadError, type BoardEventsBatch, type BoardEventsReadModel, type ExplorerEvent } from "../../application/data/explorer-read-model";
import type { Locale } from "../../platform/preferences/preferences"
import { taskOpenerKey } from "../../platform/focus/explorer-focus"
import type { WebRuntimeConfig } from "../../lib/runtime"
import { usePreferences } from "../../platform/preferences/use-preferences"

import styles from "./EventsView.module.css"

export type EventsReadState = {
  readonly data: BoardEventsReadModel | null
  readonly loading: boolean
  readonly error: ExplorerReadError | Error | null
  readonly stale: boolean
}

export interface EventsPresentationProps {
  readonly locale: Locale
  readonly taskId: string | null
  readonly kindFilter: string
  readonly state: EventsReadState
  readonly online: boolean
  readonly onRefresh: () => void
  readonly onKindFilterChange?: (value: string) => void
  readonly onSelectTask?: (taskId: string) => void
}

export interface EventsViewProps {
  readonly runtime: WebRuntimeConfig
  readonly boardSelector: string
  /** Explorer 已有的 task query；同时作为可选的看板事件筛选。 */
  readonly taskId?: string | null
  readonly kindFilter?: string
  /** 由现有 persistent sync owner 递增，用于触发 catch-up read。 */
  readonly invalidationRevision?: number
  /** 仅 recovery/gap/poll boundaries 递增；普通 SSE event 通过 batch 进入。 */
  readonly eventsRefreshRevision?: number
  /** 该 sync owner 提供的可选已校验 batch；本组件不会打开 stream。 */
  readonly batch?: BoardEventsBatch | null
  readonly online?: boolean
  readonly onKindFilterChange?: (value: string) => void
  readonly onSelectTask?: (taskId: string) => void
}

type EventsCopy = {
  readonly title: string
  readonly kicker: string
  readonly board: string
  readonly task: string
  readonly kind: string
  readonly run: string
  readonly time: string
  readonly actor: string
  readonly kindFilter: string
  readonly kindFilterPlaceholder: string
  readonly loading: string
  readonly empty: string
  readonly noMatches: string
  readonly error: string
  readonly offline: string
  readonly offlineDescription: string
  readonly degraded: string
  readonly degradedDescription: string
  readonly retry: string
  readonly refresh: string
  readonly apply: string
  readonly refreshError: string
  readonly count: (count: number) => string
  readonly unknown: string
}

const copies: Record<Locale, EventsCopy> = {
  zh: {
    title: "事件",
    kicker: "看板事件",
    board: "看板",
    task: "任务",
    kind: "类型",
    run: "运行",
    time: "时间",
    actor: "执行者",
    kindFilter: "事件类型筛选",
    kindFilterPlaceholder: "例如 task.updated",
    loading: "正在加载事件…",
    empty: "当前看板暂无事件。",
    noMatches: "没有匹配当前筛选的事件。",
    error: "事件加载失败",
    offline: "当前离线",
    offlineDescription: "连接 kanban serve 后可以重新加载事件。",
    degraded: "事件数据可能已过期",
    degradedDescription: "保留最近一次可用结果；请刷新以确认最新状态。",
    retry: "重试",
    refresh: "刷新",
    apply: "应用",
    refreshError: "刷新失败，仍显示旧数据。",
    count: (count) => `${count} 条事件`,
    unknown: "—",
  },
  en: {
    title: "Events",
    kicker: "BOARD EVENTS",
    board: "Board",
    task: "Task",
    kind: "Kind",
    run: "Run",
    time: "Time",
    actor: "Actor",
    kindFilter: "Event kind filter",
    kindFilterPlaceholder: "for example task.updated",
    loading: "Loading events…",
    empty: "This board has no events.",
    noMatches: "No events match the current filter.",
    error: "Events failed to load",
    offline: "You are offline",
    offlineDescription: "Reconnect to kanban serve to load events.",
    degraded: "Event data may be stale",
    degradedDescription: "The last usable result is retained; refresh to confirm current state.",
    retry: "Retry",
    refresh: "Refresh",
    apply: "Apply",
    refreshError: "Refresh failed; showing the last usable result.",
    count: (count) => `${count} events`,
    unknown: "—",
  },
}

function errorKind(error: Error | null): string | null {
  if (!error || !("kind" in error)) return null
  const kind = error.kind
  return typeof kind === "string" ? kind : null
}

const eventTimeFormatters = { "zh-CN": new Intl.DateTimeFormat("zh-CN", { dateStyle: "medium", timeStyle: "short" }), "en-US": new Intl.DateTimeFormat("en-US", { dateStyle: "medium", timeStyle: "short" }) };

function eventTimestamp(value: number, locale: Locale): { readonly display: string; readonly iso: string } {
  const milliseconds = Math.abs(value) < 1_000_000_000_000 ? value * 1_000 : value
  const date = new Date(milliseconds)
  if (Number.isNaN(date.getTime())) return { display: String(value), iso: String(value) }
  const language = locale === "en" ? "en-US" : "zh-CN"
  let display: string
  try {
    display = eventTimeFormatters[language].format(date)
  } catch {
    display = date.toISOString()
  }
  return { display, iso: date.toISOString() }
}

function machineToken(value: string | null | undefined, fallback: string): ReactNode {
  return value ? <code className={styles.token} translate="no">{value}</code> : <span>{fallback}</span>
}

function EventRow({ event, copy, locale, onSelectTask }: { readonly event: ExplorerEvent; readonly copy: EventsCopy; readonly locale: Locale; readonly onSelectTask?: (taskId: string) => void }) {
  const time = eventTimestamp(event.created_at, locale)
  return <article className="activity-feed-row activity-kind-task" data-testid="event-row" data-event-id={event.event_id}>
    <span className="activity-icon"><Icon name={event.run_id ? 'play' : 'task'} size={18} /></span>
    <div><div className="activity-feed-heading"><strong>{machineToken(event.kind, copy.unknown)}</strong><time dateTime={time.iso} title={time.iso}>{time.display}</time></div>
      <p>{event.actor || copy.unknown}{event.run_id && <> · {machineToken(event.run_id, copy.unknown)}</>}</p>
      {event.task_id && onSelectTask ? <Button size="sm" variant="ghost" icon="arrowUp" data-task-opener={taskOpenerKey(event.task_id)} onClick={()=>onSelectTask(event.task_id!)}>查看任务 {event.task_id}</Button> : machineToken(event.task_id, copy.unknown)}
    </div>
  </article>;
}

export function EventsPresentation({
  locale,
  taskId,
  kindFilter,
  state,
  online,
  onRefresh,
  onKindFilterChange,
  onSelectTask,
}: EventsPresentationProps) {
  const copy = copies[locale]
  const error = state.error instanceof Error ? state.error : null
  const offline = !online || errorKind(error) === "offline"
  const scopedData = state.data && (taskId === null || state.data.taskId === taskId) ? state.data : null

  if (!scopedData) return <EventsBoundary state={state} offline={offline} error={error} copy={copy} onRefresh={onRefresh} />;

  const visibleEvents = kindFilter.trim().length === 0
    ? scopedData.events
    : scopedData.events.filter((event) => event.kind.toLocaleLowerCase().includes(kindFilter.trim().toLocaleLowerCase()))
  const degraded = state.stale || Boolean(state.error) || offline

  return <ActivityFeed locale={locale} kindFilter={kindFilter} loading={state.loading} error={state.error} degraded={degraded} offline={offline} scopedData={scopedData} visibleEvents={visibleEvents} onRefresh={onRefresh} onKindFilterChange={onKindFilterChange} onSelectTask={onSelectTask} />;
}
function EventsBoundary({state,offline,error,copy,onRefresh}:{state:EventsReadState;offline:boolean;error:Error|null;copy:EventsCopy;onRefresh:()=>void}) {
  if ((state.loading || !state.error) && !offline) {
    return <section className={styles.state} data-testid="events-loading" role="status" aria-labelledby="events-loading-heading"><h2 id="events-loading-heading">{copy.title}</h2><p>{copy.loading}</p></section>
  }
  if (offline) {
    return (
      <section className={styles.state} data-testid="events-offline" role="status">
        <h2>{copy.offline}</h2>
        <p>{copy.offlineDescription}</p>
        <button type="button" onClick={onRefresh}>{copy.retry}</button>
      </section>
    )
  }
  if (error) {
    return (
      <section className={styles.state} data-testid="events-error" role="alert">
        <h2>{copy.error}</h2>
        <p>{error.message}</p>
        <button type="button" onClick={onRefresh}>{copy.retry}</button>
      </section>
    )
  }
  return null

}
function ActivityFeed({locale,kindFilter,loading,error,degraded,offline,scopedData,visibleEvents,onRefresh,onKindFilterChange,onSelectTask}: Pick<EventsPresentationProps,'locale'|'kindFilter'|'onRefresh'|'onKindFilterChange'|'onSelectTask'> & {loading:boolean;error:EventsReadState['error'];degraded:boolean;offline:boolean;scopedData:BoardEventsReadModel;visibleEvents:readonly ExplorerEvent[]}) {
  const copy=copies[locale];
  return <section className="activity-page" data-testid={degraded ? 'events-degraded-stale' : scopedData.events.length ? 'events-ready' : 'events-empty'}>
    <PageHeader eyebrow="PROJECT ACTIVITY" title="项目动态" description="保留结构调整、任务变化与验证结论的来由。" actions={<Button size="sm" icon="reset" disabled={loading} data-testid="events-refresh" onClick={onRefresh}>{copy.refresh}</Button>} />
    <div className="page-toolbar"><Tabs value={kindFilter === 'task.' ? 'task.' : ''} onChange={value=>onKindFilterChange?.(value)} items={[{value:'',label:'全部'},{value:'map.',label:'地图结构',disabled:true},{value:'task.',label:'任务'},{value:'cycle.',label:'迭代',disabled:true},{value:'module.',label:'模块',disabled:true},{value:'evidence.',label:'验证证据',disabled:true}]} /><Badge>{visibleEvents.length} 条记录</Badge></div>
    {degraded && <div className="paper-banner" role={offline ? 'status' : 'alert'} data-testid="events-stale-notice"><strong>{offline ? copy.offline : copy.degraded}</strong><span>{offline ? copy.offlineDescription : error ? copy.refreshError : copy.degradedDescription}</span></div>}
    <div className="activity-feed">{visibleEvents.map(event=><EventRow key={event.event_id} event={event} copy={copy} locale={locale} onSelectTask={onSelectTask} />)}</div>
    {!visibleEvents.length && <p className="muted" role="status" data-testid={scopedData.events.length ? 'events-filter-empty' : undefined}>{scopedData.events.length ? copy.noMatches : copy.empty}</p>}
    <details className="paper-detail-options"><summary>事件类型筛选</summary><FilterBar key={kindFilter} copy={copy} value={kindFilter} onChange={onKindFilterChange} /></details>
  </section>;
}

function FilterBar({ copy, value, onChange }: { readonly copy: EventsCopy; readonly value: string; readonly onChange?: (value: string) => void }) {
  const [draft, setDraft] = useState(value)
  if (!onChange) return null
  return (
    <form className={styles.filterBar} onSubmit={(event) => { event.preventDefault(); onChange(draft) }}>
      <label htmlFor="events-kind-filter">{copy.kindFilter}</label>
      <input
        id="events-kind-filter"
        name="event-kind"
        type="search"
        autoComplete="off"
        value={draft}
        placeholder={copy.kindFilterPlaceholder}
        onChange={(event) => setDraft(event.currentTarget.value)}
      />
      <button type="submit">{copy.apply}</button>
    </form>
  )
}

function useBoardEventsRead(
  runtime: WebRuntimeConfig, boardSelector: string, taskId: string | null,
  eventsRefreshRevision: number, online: boolean,
): EventsReadState & { readonly refresh: () => void } {
  const { loadBoardEvents, querySubscriptions } = useWorkspaceOperations();
  const key = JSON.stringify([runtime.apiBaseUrl, runtime.webBuildId, boardSelector, taskId]);
  const read = useAsyncRead(true, key, signal => loadBoardEvents(runtime, boardSelector, { taskId, signal }),
    querySubscriptions ? 0 : eventsRefreshRevision, online);
  return { ...read, stale: Boolean(read.data && (read.error || !online)), refresh: read.retry };
}

export function EventsView({
  runtime,
  boardSelector,
  taskId = null,
  kindFilter: kindFilterProp = "",
  invalidationRevision = 0,
  eventsRefreshRevision = invalidationRevision,
  online = typeof navigator === "undefined" || navigator.onLine,
  onKindFilterChange,
  onSelectTask,
}: EventsViewProps) {
  const { locale } = usePreferences()
  const [localKindFilter, setLocalKindFilter] = useState(kindFilterProp)
  const kindFilter = onKindFilterChange ? kindFilterProp : localKindFilter
  const state = useBoardEventsRead(runtime, boardSelector, taskId, eventsRefreshRevision, online)
  const setKindFilter = (value: string) => {
    setLocalKindFilter(value)
    onKindFilterChange?.(value)
  }
  // 让非受控页面跟随浏览器 back/forward 导致的 route 变化。
  useEffect(() => {
    if (!onKindFilterChange) setLocalKindFilter(kindFilterProp)
  }, [kindFilterProp, onKindFilterChange])
  return (
    <EventsPresentation
      locale={locale}
      taskId={taskId}
      kindFilter={kindFilter}
      state={state}
      online={online}
      onRefresh={state.refresh}
      onKindFilterChange={setKindFilter}
      onSelectTask={onSelectTask}
    />
  )
}
