import { renderToStaticMarkup } from "react-dom/server"
import { describe, expect, test, vi } from "vitest"

import { asCanonicalBoardId } from "../../lib/sync/contracts"
import { parseCanonicalBoardSlug } from "../../lib/board-slug"
import type { BoardEventsReadModel, ExplorerEvent } from "../../lib/api/explorer-read-model"
import { EventsPresentation, type EventsReadState } from "./EventsView"

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
  test("renders loading, empty, error, offline, and stale boundaries", () => {
    const loading = renderToStaticMarkup(<EventsPresentation locale="zh" taskId={null} kindFilter="" state={{ data: null, loading: true, error: null, stale: false }} online onRefresh={vi.fn()} />)
    const empty = renderToStaticMarkup(<EventsPresentation locale="zh" taskId={null} kindFilter="" state={{ ...ready, data: { ...model, events: [], meta: { ...model.meta, count: 0, nextAfter: 0 } } }} online onRefresh={vi.fn()} />)
    const error = renderToStaticMarkup(<EventsPresentation locale="en" taskId={null} kindFilter="" state={{ data: null, loading: false, error: new Error("boom"), stale: false }} online onRefresh={vi.fn()} />)
    const offline = renderToStaticMarkup(<EventsPresentation locale="en" taskId={null} kindFilter="" state={{ data: null, loading: false, error: null, stale: false }} online={false} onRefresh={vi.fn()} />)
    const stale = renderToStaticMarkup(<EventsPresentation locale="zh" taskId="t_1" kindFilter="" state={{ ...ready, stale: true, error: new Error("stale") }} online onRefresh={vi.fn()} />)

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
    expect(markup).toContain(">Apply</button>")
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
})
