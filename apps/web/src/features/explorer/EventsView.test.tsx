import { renderToStaticMarkup } from "react-dom/server"
import { describe, expect, test, vi } from "vitest"

import { asCanonicalBoardId } from "../../lib/sync/contracts"
import { parseCanonicalBoardSlug } from "../../lib/board-slug"
import { ExplorerReadError, type BoardEventsReadModel, type ExplorerEvent } from "../../lib/api/explorer-read-model"
import { EventsPresentation, type EventsReadState } from "./EventsView"
import { __test } from "./EventsView.performance"

const event = (id: number, overrides: Partial<ExplorerEvent> = {}): ExplorerEvent => ({
  id,
  event_id: `event-${id}`,
  board_id: "b_default",
  task_id: "t_1",
  run_id: id % 2 === 0 ? `r_${id}` : null,
  kind: "task.updated",
  actor: "tester",
  payload: {},
  created_at: 1_700_000_000 + id,
  ...overrides,
})

const model: BoardEventsReadModel = {
  board: { selector: "default", id: asCanonicalBoardId("b_default"), slug: parseCanonicalBoardSlug("default")!, name: "Default" },
  taskId: null,
  events: [event(1), event(2)],
  meta: { count: 2, nextAfter: 2, limit: 150 },
}

const ready: EventsReadState = { data: model, loading: false, error: null, stale: false }

describe("EventsView", () => {
  test("caches timestamp formatters by locale and only formats rows that need rendering", () => {
    __test.resetEventTimestampFormatters()
    const englishFormatter = __test.eventTimestampFormatter("en")
    const format = englishFormatter.format.bind(englishFormatter)
    const formatCalls = vi.fn(format)
    Object.defineProperty(englishFormatter, "format", { configurable: true, value: formatCalls })

    try {
      __test.eventTimestamp(event(1).created_at, "en")
      __test.eventTimestamp(event(2).created_at, "en")
      __test.eventTimestamp(event(3).created_at, "zh")

      expect(__test.eventTimestampFormatter("en")).toBe(englishFormatter)
      expect(__test.eventTimestampFormatter("zh")).not.toBe(englishFormatter)
      expect(formatCalls).toHaveBeenCalledTimes(2)
    } finally {
      __test.resetEventTimestampFormatters()
    }
  })

  test("keeps event payload JSON deterministic and discloses verified evidence through keyboard-native details", () => {
    const payload = {
      zulu: "<script>alert('server')</script>",
      alpha: { zulu: "&", alpha: "</code>" },
    }
    const reorderedPayload = {
      alpha: { alpha: "</code>", zulu: "&" },
      zulu: "<script>alert('server')</script>",
    }
    expect(__test.eventPayloadJson(payload)).toBe(__test.eventPayloadJson(reorderedPayload))

    const markup = renderToStaticMarkup(
      <EventsPresentation
        locale="en"
        taskId={null}
        kindFilter=""
        state={{ ...ready, data: { ...model, events: [event(7, { event_id: "event<&", payload, task_id: "t<&", run_id: "r<&", actor: "actor<&" })] } }}
        online
        onRefresh={vi.fn()}
      />,
    )

    expect(markup).toMatch(/<details[^>]*data-testid="event-detail-disclosure"[^>]*>[\s\S]*<summary>View event evidence<\/summary>/)
    expect(markup).toContain('data-testid="event-payload-json"')
    expect(markup).toContain("Canonical ID")
    expect(markup).toContain("Event ID")
    expect(markup).toContain("Task")
    expect(markup).toContain("Run")
    expect(markup).toContain("Actor")
    expect(markup).toContain("Time")
    expect(markup).toContain("Payload")
    expect(markup).toContain("&lt;script&gt;alert(&#x27;server&#x27;)&lt;/script&gt;")
    expect(markup).not.toContain("<script>alert('server')</script>")
    expect(markup).toContain('data-language="json"')
    expect(markup.indexOf("&quot;alpha&quot;")).toBeLessThan(markup.indexOf("&quot;zulu&quot;"))
    expect(markup).not.toContain('data-testid="event-detail-disclosure" open')
  })

  test("redacts server-controlled error messages while retaining safe error context", () => {
    const secret = "server-controlled secret /internal/events"
    const readError = new ExplorerReadError("http", secret, { status: 503 })
    const markup = renderToStaticMarkup(
      <EventsPresentation
        locale="en"
        taskId={null}
        kindFilter=""
        state={{ data: null, loading: false, error: readError, stale: false }}
        online
        onRefresh={vi.fn()}
      />,
    )
    const staleMarkup = renderToStaticMarkup(
      <EventsPresentation
        locale="en"
        taskId={null}
        kindFilter=""
        state={{ ...ready, error: readError, stale: true }}
        online
        onRefresh={vi.fn()}
      />,
    )

    expect(markup).not.toContain(secret)
    expect(staleMarkup).not.toContain(secret)
    expect(markup).toContain("The event response could not be safely displayed.")
    expect(staleMarkup).toContain("The event response could not be safely displayed.")
    expect(markup).toContain("http")
    expect(markup).toContain("503")
  })

  test("treats the same event object as stable across unrelated parent rerenders", () => {
    const onSelectTask = vi.fn()
    const stableProps = { event: event(1), locale: "en" as const, onSelectTask }

    expect(__test.areEventRowPropsEqual(stableProps, { ...stableProps })).toBe(true)
    expect(__test.areEventRowPropsEqual(stableProps, { ...stableProps, event: { ...stableProps.event } })).toBe(false)
    expect(__test.areEventRowPropsEqual(stableProps, { ...stableProps, locale: "zh" })).toBe(false)
  })

  test("renders loading, empty, error, offline, and stale boundaries", () => {
    const loading = renderToStaticMarkup(<EventsPresentation locale="zh" taskId={null} kindFilter="" state={{ data: null, loading: true, error: null, stale: false }} online onRefresh={vi.fn()} />)
    const empty = renderToStaticMarkup(<EventsPresentation locale="zh" taskId={null} kindFilter="" state={{ ...ready, data: { ...model, events: [], meta: { ...model.meta, count: 0, nextAfter: 0 } } }} online onRefresh={vi.fn()} />)
    const error = renderToStaticMarkup(<EventsPresentation locale="en" taskId={null} kindFilter="" state={{ data: null, loading: false, error: new Error("boom"), stale: false }} online onRefresh={vi.fn()} />)
    const offline = renderToStaticMarkup(<EventsPresentation locale="en" taskId={null} kindFilter="" state={{ data: null, loading: false, error: null, stale: false }} online={false} onRefresh={vi.fn()} />)
    const stale = renderToStaticMarkup(<EventsPresentation locale="zh" taskId="t_1" kindFilter="" state={{ ...ready, data: { ...model, taskId: "t_1" }, stale: true, error: new Error("stale") }} online onRefresh={vi.fn()} />)

    expect(loading).toContain('data-testid="events-loading"')
    expect(empty).toContain('data-testid="events-empty"')
    expect(error).toContain('data-testid="events-error"')
    expect(offline).toContain('data-testid="events-offline"')
    expect(stale).toContain('data-testid="events-degraded-stale"')
    expect(empty).toContain("<h2")
    expect(error).toContain("<h2")
    expect(offline).toContain("<h2")
    expect(empty).not.toContain("<h1")
    expect(stale).not.toContain("<h1")
  })

  test("renders machine tokens, accessible ISO time, and keyboard task links", () => {
    const markup = renderToStaticMarkup(<EventsPresentation locale="en" taskId={null} kindFilter="" state={ready} online onRefresh={vi.fn()} onSelectTask={vi.fn()} onKindFilterChange={vi.fn()} />)

    expect(markup).toContain('data-testid="events-ready"')
    expect(markup).toContain('data-testid="event-row"')
    expect(markup).toContain('translate="no"')
    expect(markup).toContain('dateTime="2023-11-14T22:13:21.000Z"')
    expect(markup).toContain('type="button"')
    expect(markup).toContain("<form")
    expect(markup).toContain("Apply")
  })

  test("filters kinds without treating the filtered result as an API empty state", () => {
    const markup = renderToStaticMarkup(<EventsPresentation locale="zh" taskId={null} kindFilter="missing" state={ready} online onRefresh={vi.fn()} onKindFilterChange={vi.fn()} />)
    expect(markup).toContain('data-testid="events-filter-empty"')
    expect(markup).not.toContain('data-testid="events-empty"')
  })

  test("keeps the deferred task opener row mounted while the unfiltered snapshot reloads", () => {
    const deferredSnapshot = { ...model, taskId: "t_1" }
    const markup = renderToStaticMarkup(
      <EventsPresentation
        locale="zh"
        taskId={null}
        kindFilter=""
        state={{ data: deferredSnapshot, loading: true, error: null, stale: true }}
        online
        onRefresh={vi.fn()}
        onSelectTask={vi.fn()}
      />,
    )

    expect(markup).toContain('data-testid="events-degraded-stale"')
    expect(markup).toContain('data-task-opener="t_1"')
    expect(markup).toContain('data-testid="event-row"')
  })

  test("does not render a retained task-A snapshot while task-B is loading", () => {
    const markup = renderToStaticMarkup(
      <EventsPresentation
        locale="en"
        taskId="t_2"
        kindFilter=""
        state={{ ...ready, data: { ...model, taskId: "t_1" }, loading: true, stale: true }}
        online
        onRefresh={vi.fn()}
      />,
    )

    expect(markup).toContain('data-testid="events-loading"')
    expect(markup).not.toContain('data-testid="event-row"')
  })

  test("does not present a stale board snapshot as task-filtered data", () => {
    const markup = renderToStaticMarkup(
      <EventsPresentation
        locale="en"
        taskId="t_2"
        kindFilter=""
        state={{ ...ready, stale: true, error: new Error("task read failed") }}
        online
        onRefresh={vi.fn()}
      />,
    )

    expect(markup).toContain('data-testid="events-error"')
    expect(markup).not.toContain('data-testid="event-row"')
  })
})
