import { useEffect, useRef, useState, type ReactNode } from "react"

import {
  ExplorerReadError,
  loadBoardEvents,
  mergeBoardEvents,
  type BoardEventsBatch,
  type BoardEventsReadModel,
  type ExplorerEvent,
} from "../../lib/api/events-read-model"
import type { Locale } from "../../lib/preferences"
import { taskOpenerKey } from "../../lib/explorer-focus"
import type { WebRuntimeConfig } from "../../lib/runtime"
import { usePreferences } from "../../lib/use-preferences"

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
    kicker: "BOARD EVENTS",
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

function eventTimestamp(value: number, locale: Locale): { readonly display: string; readonly iso: string } {
  const milliseconds = Math.abs(value) < 1_000_000_000_000 ? value * 1_000 : value
  const date = new Date(milliseconds)
  if (Number.isNaN(date.getTime())) return { display: String(value), iso: String(value) }
  const language = locale === "en" ? "en-US" : "zh-CN"
  let display: string
  try {
    display = new Intl.DateTimeFormat(language, { dateStyle: "medium", timeStyle: "short" }).format(date)
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
  return (
    <tr data-testid="event-row" data-event-id={event.event_id}>
      <td>{machineToken(event.kind, copy.unknown)}</td>
      <td>
        {event.task_id && onSelectTask ? (
          <button type="button" className={styles.tokenButton} data-task-opener={taskOpenerKey(event.task_id as string)} onClick={() => onSelectTask(event.task_id as string)}>
            {machineToken(event.task_id, copy.unknown)}
          </button>
        ) : machineToken(event.task_id, copy.unknown)}
      </td>
      <td>{machineToken(event.run_id, copy.unknown)}</td>
      <td>
        <time dateTime={time.iso} title={time.iso}>{time.display}</time>
      </td>
      <td>{machineToken(event.actor, copy.unknown)}</td>
    </tr>
  )
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

  if (!state.data && (state.loading || !state.error) && !offline) {
    return <section className={styles.state} data-testid="events-loading" role="status" aria-labelledby="events-loading-heading"><h2 id="events-loading-heading">{copy.title}</h2><p>{copy.loading}</p></section>
  }
  if (!state.data && offline) {
    return (
      <section className={styles.state} data-testid="events-offline" role="status">
        <h2>{copy.offline}</h2>
        <p>{copy.offlineDescription}</p>
        <button type="button" onClick={onRefresh}>{copy.retry}</button>
      </section>
    )
  }
  if (!state.data && error) {
    return (
      <section className={styles.state} data-testid="events-error" role="alert">
        <h2>{copy.error}</h2>
        <p>{error.message}</p>
        <button type="button" onClick={onRefresh}>{copy.retry}</button>
      </section>
    )
  }
  if (!state.data) return null

  const visibleEvents = kindFilter.trim().length === 0
    ? state.data.events
    : state.data.events.filter((event) => event.kind.toLocaleLowerCase().includes(kindFilter.trim().toLocaleLowerCase()))
  const degraded = state.stale || Boolean(state.error) || offline

  if (state.data.events.length === 0 && !state.error) {
    return (
      <section className={styles.events} data-testid="events-empty" aria-labelledby="events-heading">
        <header className={styles.heading}>
          <p className={styles.kicker}>{copy.kicker}</p>
          <h2 id="events-heading">{copy.title}</h2>
          <p className={styles.boardContext}>{copy.board} · {machineToken(state.data.board.slug, copy.unknown)}</p>
          <button type="button" onClick={onRefresh}>{copy.refresh}</button>
        </header>
        <FilterBar copy={copy} value={kindFilter} onChange={onKindFilterChange} />
        <p className={styles.empty} role="status">{copy.empty}</p>
      </section>
    )
  }

  return (
    <section className={styles.events} data-testid={degraded ? "events-degraded-stale" : "events-ready"} aria-labelledby="events-heading">
      <header className={styles.heading}>
        <div>
          <p className={styles.kicker}>{copy.kicker}</p>
          <h2 id="events-heading">{copy.title}</h2>
          <p className={styles.boardContext}>{copy.board} · {machineToken(state.data.board.slug, copy.unknown)}</p>
          {taskId ? <p className={styles.boardContext}>{copy.task} · {machineToken(taskId, copy.unknown)}</p> : null}
        </div>
        <button type="button" data-testid="events-refresh" onClick={onRefresh} disabled={state.loading}>{copy.refresh}</button>
      </header>
      {degraded ? (
        <div className={styles.notice} role={offline ? "status" : "alert"} data-testid="events-stale-notice">
          <strong>{offline ? copy.offline : copy.degraded}</strong>
          <span>{offline ? copy.offlineDescription : state.error ? copy.refreshError : copy.degradedDescription}</span>
        </div>
      ) : null}
      {state.error && !offline ? <p className={styles.errorDetail} role="alert">{state.error.message}</p> : null}
      <FilterBar copy={copy} value={kindFilter} onChange={onKindFilterChange} />
      {visibleEvents.length === 0 ? (
        <p className={styles.empty} role="status" data-testid="events-filter-empty">{copy.noMatches}</p>
      ) : (
        <div className={styles.tableRegion} role="region" aria-label={copy.title} tabIndex={0}>
          <table className={styles.table}>
            <caption className={styles.visuallyHidden}>{copy.title}</caption>
            <thead>
              <tr>
                <th scope="col">{copy.kind}</th>
                <th scope="col">{copy.task}</th>
                <th scope="col">{copy.run}</th>
                <th scope="col">{copy.time}</th>
                <th scope="col">{copy.actor}</th>
              </tr>
            </thead>
            <tbody>
              {visibleEvents.map((event) => <EventRow key={`${event.id}:${event.event_id}`} event={event} copy={copy} locale={locale} onSelectTask={onSelectTask} />)}
            </tbody>
          </table>
        </div>
      )}
      <p className={styles.count} aria-live="polite">{copy.count(visibleEvents.length)}</p>
    </section>
  )
}

function FilterBar({ copy, value, onChange }: { readonly copy: EventsCopy; readonly value: string; readonly onChange?: (value: string) => void }) {
  const [draft, setDraft] = useState(value)
  useEffect(() => setDraft(value), [value])
  if (!onChange) return null
  return (
    <form className={styles.filterBar} onSubmit={(event) => { event.preventDefault(); onChange(draft) }}>
      <label htmlFor="events-kind-filter">{copy.kindFilter}</label>
      <input
        id="events-kind-filter"
        type="search"
        value={draft}
        placeholder={copy.kindFilterPlaceholder}
        onChange={(event) => setDraft(event.currentTarget.value)}
      />
      <button type="submit">{copy.apply}</button>
    </form>
  )
}

function useBoardEventsRead(
  runtime: WebRuntimeConfig,
  boardSelector: string,
  taskId: string | null,
  invalidationRevision: number,
  batch: BoardEventsBatch | null | undefined,
  online: boolean,
): EventsReadState & { readonly refresh: () => void } {
  const identityKey = JSON.stringify([
    runtime.apiBaseUrl,
    runtime.webBasePath,
    runtime.webBuildId,
    boardSelector,
    taskId,
  ])
  const loadRef = useRef<(signal: AbortSignal) => Promise<BoardEventsReadModel>>((signal) => loadBoardEvents(runtime, boardSelector, { taskId, signal }))
  loadRef.current = (signal) => loadBoardEvents(runtime, boardSelector, { taskId, signal })
  const [generation, setGeneration] = useState(0)
  const requestKey = `${identityKey}\u0000${invalidationRevision}\u0000${generation}`
  type InternalEventsReadState = EventsReadState & {
    readonly identityKey: string
    readonly requestKey: string
  }
  const [state, setState] = useState<InternalEventsReadState>(() => ({
    identityKey,
    requestKey,
    data: null,
    loading: false,
    error: null,
    stale: false,
  }))

  useEffect(() => {
    const requestIdentity = identityKey
    const requestToken = requestKey
    if (!online) {
      setState((current) => ({
        identityKey: requestIdentity,
        requestKey: requestToken,
        data: current.identityKey === requestIdentity ? current.data : null,
        loading: false,
        error: new ExplorerReadError("offline", "当前离线，无法加载事件。"),
        stale: current.identityKey === requestIdentity && current.data !== null,
      }))
      return
    }
    const controller = new AbortController()
    let active = true
    setState((current) => ({
      identityKey: requestIdentity,
      requestKey: requestToken,
      data: current.identityKey === requestIdentity ? current.data : null,
      loading: true,
      error: null,
      stale: current.identityKey === requestIdentity && current.data !== null,
    }))
    void loadRef.current(controller.signal).then(
      (data) => {
        if (!active || controller.signal.aborted) return
        setState((current) => {
          if (current.identityKey !== requestIdentity || current.requestKey !== requestToken) return current
          if (current.data && current.data.board.id === data.board.id && current.data.taskId === data.taskId) {
            const events = mergeBoardEvents(current.data.events, data.events, data.board.id)
            return {
              identityKey: requestIdentity,
              requestKey: requestToken,
              data: { ...data, events, meta: { ...data.meta, count: events.length, nextAfter: Math.max(current.data.meta.nextAfter, data.meta.nextAfter) } },
              loading: false,
              error: null,
              stale: false,
            }
          }
          return { identityKey: requestIdentity, requestKey: requestToken, data, loading: false, error: null, stale: false }
        })
      },
      (error: unknown) => {
        if (!active || controller.signal.aborted || (error instanceof Error && error.name === "AbortError")) return
        setState((current) => {
          if (current.identityKey !== requestIdentity || current.requestKey !== requestToken) return current
          return {
            identityKey: requestIdentity,
            requestKey: requestToken,
            data: current.data,
            loading: false,
            error: error instanceof Error ? error : new Error(String(error)),
            stale: current.data !== null,
          }
        })
      },
    )
    return () => {
      active = false
      controller.abort()
    }
  }, [boardSelector, generation, identityKey, invalidationRevision, online, requestKey, taskId])

  useEffect(() => {
    if (!batch) return
    setState((current) => {
      if (current.identityKey !== identityKey || !current.data || current.data.board.id !== batch.boardId) return current
      try {
        if (!Number.isSafeInteger(batch.nextAfter) || batch.nextAfter < 0) {
          throw new ExplorerReadError("anomaly", "事件 batch 的 nextAfter 不是非负安全整数。")
        }
        let previousId = -1
        let maxIncomingId = -1
        for (const event of batch.events) {
          if (!Number.isSafeInteger(event.id) || event.id <= 0 || event.id <= previousId) {
            throw new ExplorerReadError("anomaly", "事件 batch 的 id 必须严格递增。")
          }
          if (event.board_id !== batch.boardId) {
            throw new ExplorerReadError("anomaly", "事件 batch 越过当前 board scope。")
          }
          if (event.event_id.trim().length === 0) {
            throw new ExplorerReadError("anomaly", "事件 batch 缺少 event_id。")
          }
          if (event.id > batch.nextAfter) {
            throw new ExplorerReadError("anomaly", "事件 batch 的 id 不得超过 nextAfter。")
          }
          previousId = event.id
          maxIncomingId = event.id
        }
        if (batch.nextAfter < current.data.meta.nextAfter) {
          throw new ExplorerReadError("anomaly", "事件 batch 的 nextAfter 不得回退。")
        }
        if (batch.events.length === 0) {
          if (batch.nextAfter !== current.data.meta.nextAfter) {
            throw new ExplorerReadError("anomaly", "空事件 batch 不得推进 nextAfter。")
          }
        } else if (batch.nextAfter !== maxIncomingId) {
          throw new ExplorerReadError("anomaly", "事件 batch 的 nextAfter 必须等于最后一个事件 id。")
        }
        const incoming = taskId === null ? batch.events : batch.events.filter((event) => event.task_id === taskId)
        const events = mergeBoardEvents(current.data.events, incoming, batch.boardId)
        return {
          ...current,
          data: { ...current.data, events, meta: { ...current.data.meta, count: events.length, nextAfter: Math.max(current.data.meta.nextAfter, batch.nextAfter) } },
          error: null,
          stale: false,
        }
      } catch (error) {
        return { ...current, error: error instanceof Error ? error : new Error(String(error)), stale: true }
      }
    })
  }, [batch, identityKey, taskId])

  const visibleState: EventsReadState = state.identityKey !== identityKey
    ? {
        data: null,
        loading: online,
        error: online ? null : new ExplorerReadError("offline", "当前离线，无法加载事件。"),
        stale: false,
      }
    : state.requestKey !== requestKey
      ? {
          data: state.data,
          loading: online,
          error: online ? null : new ExplorerReadError("offline", "当前离线，无法加载事件。"),
          stale: state.data !== null,
        }
      : state

  return { ...visibleState, refresh: () => setGeneration((value) => value + 1) }
}

export function EventsView({
  runtime,
  boardSelector,
  taskId = null,
  kindFilter: kindFilterProp = "",
  invalidationRevision = 0,
  batch,
  online = typeof navigator === "undefined" || navigator.onLine,
  onKindFilterChange,
  onSelectTask,
}: EventsViewProps) {
  const { locale } = usePreferences()
  const [localKindFilter, setLocalKindFilter] = useState(kindFilterProp)
  const kindFilter = onKindFilterChange ? kindFilterProp : localKindFilter
  const state = useBoardEventsRead(runtime, boardSelector, taskId, invalidationRevision, batch, online)
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
