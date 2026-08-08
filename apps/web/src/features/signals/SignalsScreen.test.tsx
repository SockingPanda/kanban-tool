import { renderToStaticMarkup } from "react-dom/server"
import { describe, expect, test } from "vitest"

import type { SignalRecord } from "../../lib/api/signals-ontology-read-model"
import type { SignalsRouteFilters } from "../../lib/router"

import { reconcileSelection, SignalDetailView, SignalsScreenView, type ReadState } from "./SignalsScreen"

const signal = (overrides: Partial<SignalRecord> = {}): SignalRecord => ({
  id: "sig_1",
  board_id: "b_1",
  observation_id: "obs_1",
  kind: "agent_cli_friction",
  title: "CLI friction",
  summary: "A command was rejected.",
  severity: "info",
  status: "open",
  dedupe_key: "dedupe",
  superseded_by_signal_id: null,
  reviewed_by: null,
  reviewed_at: null,
  review_reason: null,
  created_at: 1,
  updated_at: 1,
  observation: {
    id: "obs_1",
    board_id: "b_1",
    task_id: "t_1",
    task_ref_snapshot: "default#1",
    run_id: null,
    comment_id: null,
    actor: "codex",
    agent_type: "executor",
    source: "api-test",
    evidence: { command: "kanban task create", exit_code: 1 },
    created_at: 1,
  },
  ...overrides,
})

const filters: SignalsRouteFilters = { status: "review", kinds: ["agent_cli_friction"], task: "default#1" }

function state<T>(data: T, phase: ReadState<T>["phase"] = "success", error?: unknown): ReadState<T> {
  return { phase, data, error: error ?? null }
}

describe("Signals screen presentation", () => {
  test("keeps a closed detail closed while reconciling stale selected ids", () => {
    expect(reconcileSelection(null, [signal()])).toBeNull()
    expect(reconcileSelection("sig_missing", [signal()])).toBe("sig_1")
    expect(reconcileSelection("sig_1", [signal()])).toBe("sig_1")
  })

  test("renders the generic shell, filters, rows, and selected detail", () => {
    const html = renderToStaticMarkup(
      <SignalsScreenView
        boardName="Default"
        filters={filters}
        list={state([signal()])}
        detail={state(signal())}
        selectedSignalId="sig_1"
        online
        onRefresh={() => undefined}
        onFiltersChange={() => undefined}
        onSelectSignal={() => undefined}
        onCloseDetail={() => undefined}
      />,
    )

    expect(html).toContain("Signals")
    expect(html).toContain("Generic agent and product signals for the active board.")
    expect(html).toContain("Open + confirmed")
    expect(html).toContain("CLI friction")
    expect(html).toContain("Evidence JSON")
    expect(html).toContain("default#1")
    expect(html).toContain("Close detail")
  })

  test("keeps stale rows visible while a refresh fails", () => {
    const html = renderToStaticMarkup(
      <SignalsScreenView
        boardName="Default"
        filters={{}}
        list={state([signal()], "error", new Error("serve offline"))}
        detail={state(signal())}
        selectedSignalId="sig_1"
        online
        onRefresh={() => undefined}
        onFiltersChange={() => undefined}
        onSelectSignal={() => undefined}
      />,
    )

    expect(html).toContain("Unable to load signals")
    expect(html).toContain("CLI friction")
    expect(html).toContain("stale")
  })

  test("makes empty and offline states explicit without dropping the detail contract", () => {
    const html = renderToStaticMarkup(
      <SignalsScreenView
        boardName="Default"
        filters={{}}
        list={state([])}
        detail={state(null, "idle")}
        selectedSignalId={null}
        online={false}
        onRefresh={() => undefined}
        onFiltersChange={() => undefined}
        onSelectSignal={() => undefined}
      />,
    )

    expect(html).toContain("You are offline")
    expect(html).toContain("No signals returned.")
    expect(html).toContain("Select a signal to inspect observation and evidence.")
  })

  test("renders a detail not-found boundary separately from a list", () => {
    const html = renderToStaticMarkup(
      <SignalDetailView loading={false} signal={null} error={{ status: 404 }} />,
    )
    expect(html).toContain("Signal is no longer available")
  })
})
