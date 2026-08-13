import { memo, useCallback, useEffect, useRef, useState, type ReactNode } from "react"

import { Banner } from "@astryxdesign/core/Banner"
import { Button } from "@astryxdesign/core/Button"
import { Heading } from "@astryxdesign/core/Heading"
import { Table, TableBody, TableCell, TableHeader, TableHeaderCell, TableRow } from "@astryxdesign/core/Table"
import { Text } from "@astryxdesign/core/Text"

import { CodeBlock, PageFrame, SafeHStack, SafeVStack, TextInput } from "@/ui/astryx"

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
import { areEventRowPropsEqual, eventPayloadJson, eventTimestamp } from "./EventsView.performance"

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
  readonly board: string
  readonly task: string
  readonly kind: string
  readonly run: string
  readonly time: string
  readonly actor: string
  readonly id: string
  readonly eventId: string
  readonly payload: string
  readonly details: string
  readonly showDetails: string
  readonly copy: string
  readonly copied: string
  readonly copyError: string
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
  readonly unreadable: string
  readonly count: (count: number) => string
  readonly unknown: string
}

const copies: Record<Locale, EventsCopy> = {
  zh: {
    title: "事件",
    board: "看板",
    task: "任务",
    kind: "类型",
    run: "运行",
    time: "时间",
    actor: "执行者",
    id: "ID",
    eventId: "事件 ID",
    payload: "Payload",
    details: "证据",
    showDetails: "查看事件证据",
    copy: "复制",
    copied: "已复制",
    copyError: "复制失败",
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
    unreadable: "事件响应无法安全显示。",
    count: (count) => `${count} 条事件`,
    unknown: "—",
  },
  en: {
    title: "Events",
    board: "Board",
    task: "Task",
    kind: "Kind",
    run: "Run",
    time: "Time",
    actor: "Actor",
    id: "Canonical ID",
    eventId: "Event ID",
    payload: "Payload",
    details: "Evidence",
    showDetails: "View event evidence",
    copy: "Copy",
    copied: "Copied",
    copyError: "Copy failed",
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
    unreadable: "The event response could not be safely displayed.",
    count: (count) => `${count} events`,
    unknown: "—",
  },
}

function errorKind(error: Error | null): string | null {
  if (!error || !("kind" in error)) return null
  const kind = error.kind
  return typeof kind === "string" ? kind : null
}

const safeExplorerErrorKinds: ReadonlySet<ExplorerReadError["kind"]> = new Set([
  "empty",
  "offline",
  "http",
  "invalid_json",
  "invalid_contract",
  "anomaly",
  "cross_origin",
  "malformed_url",
  "invalid_content_type",
  "response_too_large",
])

function safeErrorDescription(error: Error | null, copy: EventsCopy): string {
  if (!(error instanceof ExplorerReadError)) return copy.unreadable
  const statusValue = error.status
  const status = statusValue !== null && Number.isInteger(statusValue) && statusValue >= 100 && statusValue <= 599
    ? String(statusValue)
    : null
  const kind = safeExplorerErrorKinds.has(error.kind) ? error.kind : null
  const context = [kind, status].filter(Boolean).join(" · ")
  return context.length > 0 ? `${copy.unreadable} (${context})` : copy.unreadable
}

function machineToken(value: string | null | undefined, fallback: string): ReactNode {
  return value ? <code translate="no"><Text type="code">{value}</Text></code> : <Text type="supporting">{fallback}</Text>
}

type EventRowProps = {
  readonly event: ExplorerEvent
  readonly copy: EventsCopy
  readonly locale: Locale
  readonly onSelectTask?: (taskId: string) => void
}

function EventRowContent({ event, copy, locale, onSelectTask }: EventRowProps) {
  const time = eventTimestamp(event.created_at, locale)
  const payload = eventPayloadJson(event.payload)
  return (
    <TableRow data-testid="event-row" data-event-id={event.event_id}>
      <TableCell>{machineToken(event.kind, copy.unknown)}</TableCell>
      <TableCell>
        {event.task_id && onSelectTask ? (
          <Button
            type="button"
            label={event.task_id}
            variant="ghost"
            size="sm"
            data-task-opener={taskOpenerKey(event.task_id as string)}
            onClick={() => onSelectTask(event.task_id as string)}
          >
            {machineToken(event.task_id, copy.unknown)}
          </Button>
        ) : machineToken(event.task_id, copy.unknown)}
      </TableCell>
      <TableCell>{machineToken(event.run_id, copy.unknown)}</TableCell>
      <TableCell>
        <time dateTime={time.iso} title={time.iso}>{time.display}</time>
      </TableCell>
      <TableCell>{machineToken(event.actor, copy.unknown)}</TableCell>
      <TableCell>
        <details data-testid="event-detail-disclosure">
          <summary>{copy.showDetails}</summary>
          <SafeVStack as="section" gap={2} padding={3} data-testid="event-detail-content">
            <Text as="p" type="supporting">{copy.id}: {machineToken(String(event.id), copy.unknown)}</Text>
            <Text as="p" type="supporting">{copy.eventId}: {machineToken(event.event_id, copy.unknown)}</Text>
            <Text as="p" type="supporting">{copy.task}: {machineToken(event.task_id, copy.unknown)}</Text>
            <Text as="p" type="supporting">{copy.run}: {machineToken(event.run_id, copy.unknown)}</Text>
            <Text as="p" type="supporting">{copy.actor}: {machineToken(event.actor, copy.unknown)}</Text>
            <Text as="p" type="supporting">{copy.time}: <time dateTime={time.iso} title={time.iso}>{time.display}</time></Text>
            <CodeBlock
              code={payload}
              language="json"
              isWrapped
              container="section"
              maxHeight="evidence"
              label={copy.payload}
              hasCopy={false}
              copyLabel={copy.copy}
              copiedLabel={copy.copied}
              errorLabel={copy.copyError}
              data-testid="event-payload-json"
            />
          </SafeVStack>
        </details>
      </TableCell>
    </TableRow>
  )
}

const EventRow = memo(EventRowContent, areEventRowPropsEqual)

function StateBoundary({
  testId,
  role,
  title,
  description,
  retry,
}: {
  readonly testId: string
  readonly role: "status" | "alert"
  readonly title: string
  readonly description: string
  readonly retry?: { readonly label: string; readonly onClick: () => void }
}) {
  return (
    <SafeVStack as="section" gap={2} padding={4} data-testid={testId} role={role}>
      <PageFrame
        frame="content"
        aria-labelledby={`${testId}-heading`}
        bodyLabel={title}
        header={<Heading level={2} id={`${testId}-heading`}>{title}</Heading>}
      >
        <SafeVStack gap={2}>
          <Text as="p" type="supporting">{description}</Text>
          {retry ? <Button label={retry.label} variant="secondary" size="sm" onClick={retry.onClick} /> : null}
        </SafeVStack>
      </PageFrame>
    </SafeVStack>
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
  const scopedData = state.data && (taskId === null || state.data.taskId === taskId) ? state.data : null
  const onSelectTaskRef = useRef(onSelectTask)
  onSelectTaskRef.current = onSelectTask
  const stableOnSelectTask = useCallback((selectedTaskId: string) => {
    onSelectTaskRef.current?.(selectedTaskId)
  }, [])
  const eventTaskSelection = onSelectTask ? stableOnSelectTask : undefined

  if (!scopedData && (state.loading || !state.error) && !offline) {
    return <StateBoundary testId="events-loading" role="status" title={copy.title} description={copy.loading} />
  }
  if (!scopedData && offline) {
    return <StateBoundary testId="events-offline" role="status" title={copy.offline} description={copy.offlineDescription} retry={{ label: copy.retry, onClick: onRefresh }} />
  }
  if (!scopedData && error) {
    return <StateBoundary testId="events-error" role="alert" title={copy.error} description={safeErrorDescription(error, copy)} retry={{ label: copy.retry, onClick: onRefresh }} />
  }
  if (!scopedData) return null

  const visibleEvents = kindFilter.trim().length === 0
    ? scopedData.events
    : scopedData.events.filter((event) => event.kind.toLocaleLowerCase().includes(kindFilter.trim().toLocaleLowerCase()))
  const degraded = state.stale || Boolean(state.error) || offline

  if (scopedData.events.length === 0 && !state.error) {
    return (
      <PageFrame
        frame="content"
        data-testid="events-empty"
        aria-labelledby="events-heading"
        bodyLabel={copy.title}
        header={(
          <SafeHStack as="section" justify="between" align="end" gap={4} wrap="wrap">
            <SafeVStack gap={1}>
              <Heading level={2} id="events-heading">{copy.title}</Heading>
              <Text type="supporting">{copy.board} · {machineToken(scopedData.board.slug, copy.unknown)}</Text>
            </SafeVStack>
            <Button label={copy.refresh} variant="secondary" size="sm" onClick={onRefresh} />
          </SafeHStack>
        )}
      >
        <SafeVStack gap={4}>
          <FilterBar copy={copy} value={kindFilter} onChange={onKindFilterChange} />
          <Text as="p" type="supporting" role="status">{copy.empty}</Text>
        </SafeVStack>
      </PageFrame>
    )
  }

  return (
    <PageFrame
      frame="content"
      data-testid={degraded ? "events-degraded-stale" : "events-ready"}
      aria-labelledby="events-heading"
      bodyLabel={copy.title}
      header={(
        <SafeHStack as="section" justify="between" align="end" gap={4} wrap="wrap">
          <SafeVStack gap={1}>
            <Heading level={2} id="events-heading">{copy.title}</Heading>
            <Text type="supporting">{copy.board} · {machineToken(scopedData.board.slug, copy.unknown)}</Text>
            {taskId ? <Text type="supporting">{copy.task} · {machineToken(taskId, copy.unknown)}</Text> : null}
          </SafeVStack>
          <Button label={copy.refresh} variant="secondary" size="sm" data-testid="events-refresh" onClick={onRefresh} isDisabled={state.loading} />
        </SafeHStack>
      )}
    >
      <SafeVStack gap={4}>
      {degraded ? (
        <Banner
          status={offline ? "info" : state.error ? "error" : "warning"}
          title={offline ? copy.offline : copy.degraded}
          description={offline ? copy.offlineDescription : state.error ? copy.refreshError : copy.degradedDescription}
          container="section"
          role={offline ? "status" : "alert"}
          data-testid="events-stale-notice"
          endContent={<Button label={copy.refresh} variant="ghost" size="sm" onClick={onRefresh} isDisabled={state.loading} />}
        />
      ) : null}
      {state.error && !offline ? <Text as="p" type="supporting" role="alert">{safeErrorDescription(error, copy)}</Text> : null}
      <FilterBar copy={copy} value={kindFilter} onChange={onKindFilterChange} />
      {visibleEvents.length === 0 ? (
        <Text as="p" type="supporting" role="status" data-testid="events-filter-empty">{copy.noMatches}</Text>
      ) : (
        <SafeVStack as="section" isScrollable className="min-w-0" role="region" aria-label={copy.title} tabIndex={0}>
          <Table density="compact" dividers="rows" hasHover verticalAlign="top" textOverflow="wrap" aria-label={copy.title}>
            <caption className="sr-only">{copy.title}</caption>
            <TableHeader>
              <TableRow isHeaderRow>
                <TableHeaderCell scope="col">{copy.kind}</TableHeaderCell>
                <TableHeaderCell scope="col">{copy.task}</TableHeaderCell>
                <TableHeaderCell scope="col">{copy.run}</TableHeaderCell>
                <TableHeaderCell scope="col">{copy.time}</TableHeaderCell>
                <TableHeaderCell scope="col">{copy.actor}</TableHeaderCell>
                <TableHeaderCell scope="col">{copy.details}</TableHeaderCell>
              </TableRow>
            </TableHeader>
            <TableBody>
              {visibleEvents.map((event) => <EventRow key={`${event.id}:${event.event_id}`} event={event} copy={copy} locale={locale} onSelectTask={eventTaskSelection} />)}
            </TableBody>
          </Table>
        </SafeVStack>
      )}
      <Text as="p" type="supporting" aria-live="polite">{copy.count(visibleEvents.length)}</Text>
      </SafeVStack>
    </PageFrame>
  )
}

function FilterBar({ copy, value, onChange }: { readonly copy: EventsCopy; readonly value: string; readonly onChange?: (value: string) => void }) {
  const [draft, setDraft] = useState(value)
  useEffect(() => setDraft(value), [value])
  if (!onChange) return null
  return (
    <SafeHStack as="form" gap={2} align="end" wrap="wrap" aria-label={copy.kindFilter} onSubmit={(event) => { event.preventDefault(); onChange(draft) }}>
      <TextInput
        id="events-kind-filter"
        htmlName="event-kind"
        type="search"
        autoComplete="off"
        value={draft}
        label={copy.kindFilter}
        isLabelHidden
        placeholder={copy.kindFilterPlaceholder}
        onChange={(nextValue) => setDraft(nextValue)}
      />
      <Button type="submit" label={copy.apply} variant="secondary" size="sm" />
    </SafeHStack>
  )
}

function useBoardEventsRead(
  runtime: WebRuntimeConfig,
  boardSelector: string,
  taskId: string | null,
  eventsRefreshRevision: number,
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
  const scopeKey = JSON.stringify([
    runtime.apiBaseUrl,
    runtime.webBasePath,
    runtime.webBuildId,
    boardSelector,
  ])
  const loadRef = useRef<(signal: AbortSignal) => Promise<BoardEventsReadModel>>((signal) => loadBoardEvents(runtime, boardSelector, { taskId, signal }))
  const cursorRef = useRef(0)
  const cursorIdentityRef = useRef(identityKey)
  loadRef.current = (signal) => loadBoardEvents(runtime, boardSelector, {
    taskId,
    signal,
    after: cursorIdentityRef.current === identityKey ? cursorRef.current : 0,
  })
  const [generation, setGeneration] = useState(0)
  const requestKey = `${identityKey}\u0000${eventsRefreshRevision}\u0000${generation}`
  type InternalEventsReadState = EventsReadState & {
    readonly identityKey: string
    readonly scopeKey: string
    readonly requestKey: string
  }
  const [state, setState] = useState<InternalEventsReadState>(() => ({
    identityKey,
    scopeKey,
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
        scopeKey,
        requestKey: requestToken,
        data: current.scopeKey === scopeKey ? current.data : null,
        loading: false,
        error: new ExplorerReadError("offline", "当前离线，无法加载事件。"),
        stale: current.scopeKey === scopeKey && current.data !== null,
      }))
      return
    }
    const controller = new AbortController()
    let active = true
    setState((current) => ({
      identityKey: requestIdentity,
      scopeKey,
      requestKey: requestToken,
      data: current.scopeKey === scopeKey ? current.data : null,
      loading: true,
      error: null,
      stale: current.scopeKey === scopeKey && current.data !== null,
    }))
    void loadRef.current(controller.signal).then(
      (data) => {
        if (!active || controller.signal.aborted) return
        setState((current) => {
          if (current.identityKey !== requestIdentity || current.requestKey !== requestToken) return current
          if (current.data && current.data.board.id === data.board.id && current.data.taskId === data.taskId) {
            const events = mergeBoardEvents(current.data.events, data.events, data.board.id)
            cursorRef.current = Math.max(cursorRef.current, data.meta.nextAfter)
            cursorIdentityRef.current = requestIdentity
            return {
              identityKey: requestIdentity,
              scopeKey,
              requestKey: requestToken,
              data: { ...data, events, meta: { ...data.meta, count: events.length, nextAfter: Math.max(current.data.meta.nextAfter, data.meta.nextAfter) } },
              loading: false,
              error: null,
              stale: false,
            }
          }
          cursorRef.current = data.meta.nextAfter
          cursorIdentityRef.current = requestIdentity
          return { identityKey: requestIdentity, scopeKey, requestKey: requestToken, data, loading: false, error: null, stale: false }
        })
      },
      (error: unknown) => {
        if (!active || controller.signal.aborted || (error instanceof Error && error.name === "AbortError")) return
        setState((current) => {
          if (current.identityKey !== requestIdentity || current.requestKey !== requestToken) return current
          return {
            identityKey: requestIdentity,
            scopeKey,
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
  }, [boardSelector, eventsRefreshRevision, generation, identityKey, online, requestKey, scopeKey, taskId])

  useEffect(() => {
    if (!batch) return
    setState((current) => {
      if (current.identityKey !== identityKey || !current.data || current.data.board.id !== batch.boardId || (taskId !== null && current.data.taskId !== taskId)) return current
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
        // A delayed batch can legitimately be older than the initial read;
        // it is already covered by that snapshot and must be ignored.
        if (batch.nextAfter <= current.data.meta.nextAfter) return current
        if (batch.events.length === 0) {
          if (batch.nextAfter !== current.data.meta.nextAfter) {
            throw new ExplorerReadError("anomaly", "空事件 batch 不得推进 nextAfter。")
          }
        } else if (batch.nextAfter !== maxIncomingId) {
          throw new ExplorerReadError("anomaly", "事件 batch 的 nextAfter 必须等于最后一个事件 id。")
        }
        // A batch may arrive while the initial read is still in flight. The
        // effect is retried when that read installs its snapshot below.
        const incoming = taskId === null ? batch.events : batch.events.filter((event) => event.task_id === taskId)
        const events = mergeBoardEvents(current.data.events, incoming, batch.boardId)
        cursorRef.current = Math.max(cursorRef.current, batch.nextAfter)
        cursorIdentityRef.current = identityKey
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
  }, [batch, identityKey, state.data?.meta.nextAfter, taskId])

  const visibleState: EventsReadState = state.identityKey !== identityKey
    ? {
        data: taskId === null ? state.data : null,
        loading: online,
        error: online ? null : new ExplorerReadError("offline", "当前离线，无法加载事件。"),
        stale: taskId === null && state.data !== null,
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
  eventsRefreshRevision = invalidationRevision,
  batch,
  online = typeof navigator === "undefined" || navigator.onLine,
  onKindFilterChange,
  onSelectTask,
}: EventsViewProps) {
  const { locale } = usePreferences()
  const [localKindFilter, setLocalKindFilter] = useState(kindFilterProp)
  const kindFilter = onKindFilterChange ? kindFilterProp : localKindFilter
  const state = useBoardEventsRead(runtime, boardSelector, taskId, eventsRefreshRevision, batch, online)
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
