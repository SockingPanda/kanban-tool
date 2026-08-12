import { readFileSync } from "node:fs"
import { renderToStaticMarkup } from "react-dom/server"
import { describe, expect, test } from "vitest"

import type { SignalRecord } from "../../lib/api/signals-ontology-read-model"
import { featureCopyForLocale } from "../../lib/i18n"
import type { SignalsRouteFilters } from "../../lib/router"
import { reconcileSelection, type ReadState } from "../read-state"

import { SignalDetailView, SignalsScreenView } from "./SignalsScreen"

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
    expect(html).toContain("KANBAN TOOL / SIGNALS")
    expect(html).toContain("Generic agent and product signals for the active board.")
    expect(html).toContain("Open + confirmed")
    expect(html).toContain("CLI friction")
    expect(html).toContain("Evidence JSON")
    expect(html).toContain("default#1")
    expect(html).toContain("Close detail")
    expect(html).toContain('data-columns="two"')
    expect(html).not.toContain(" style=")
    expect(html).not.toContain("<style")
  })

  test("keeps the two-column and CSP-safe text contracts in source", () => {
    const source = readFileSync(new URL("./SignalsScreen.tsx", import.meta.url), "utf8")
    expect(source).toContain('columns="two"')
    expect(source).not.toContain('columns="auto-md"')
    expect(source).not.toContain("maxLines")
    expect(source).not.toContain("@astryxdesign/core/Spinner")
    expect(source).toContain("SignalLoadingState")
    const textInputs = source.match(/<TextInput[\s\S]*?\/>/g) ?? []
    expect(textInputs).not.toHaveLength(0)
    expect(textInputs.every((input) => !/\bsize=/.test(input))).toBe(true)
  })

  test.each([
    ["zh", "信号", "刷新"],
    ["en", "Signals", "Refresh"],
  ] as const)("renders feature copy in %s while keeping machine values literal", (locale, heading, refresh) => {
    const html = renderToStaticMarkup(
      <SignalsScreenView
        boardName="Default"
        filters={filters}
        list={state([signal()])}
        detail={state(signal())}
        selectedSignalId="sig_1"
        online
        copy={featureCopyForLocale(locale).signals}
        onRefresh={() => undefined}
        onFiltersChange={() => undefined}
        onSelectSignal={() => undefined}
        onCloseDetail={() => undefined}
      />,
    )

    expect(html).toContain(heading)
    expect(html).toContain(refresh)
    expect(html).toContain('translate="no">agent_cli_friction')
    expect(html).toContain('translate="no">sig_1')
  })

  test("localizes status filter labels while preserving machine status badges", () => {
    const html = renderToStaticMarkup(
      <SignalsScreenView
        boardName="Default"
        filters={{ status: "open" }}
        list={state([signal()])}
        detail={state(signal())}
        selectedSignalId="sig_1"
        online
        copy={featureCopyForLocale("zh").signals}
        onRefresh={() => undefined}
        onFiltersChange={() => undefined}
        onSelectSignal={() => undefined}
      />,
    )

    expect(html).toContain("开放")
    expect(html).toMatch(/<span translate="no"><span[^>]*>open<\/span><\/span>/)
  })

  test("formats signal timestamps with the selected locale", () => {
    const enDate = new Date(1).toLocaleString("en-US")
    const zhDate = new Date(1).toLocaleString("zh-CN")
    const en = renderToStaticMarkup(
      <SignalsScreenView
        boardName="Default"
        filters={filters}
        list={state([signal()])}
        detail={state(signal())}
        selectedSignalId="sig_1"
        online
        locale="en"
        onRefresh={() => undefined}
        onFiltersChange={() => undefined}
        onSelectSignal={() => undefined}
      />,
    )
    const zh = renderToStaticMarkup(
      <SignalsScreenView
        boardName="Default"
        filters={filters}
        list={state([signal()])}
        detail={state(signal())}
        selectedSignalId="sig_1"
        online
        locale="zh"
        onRefresh={() => undefined}
        onFiltersChange={() => undefined}
        onSelectSignal={() => undefined}
      />,
    )
    expect(en).toContain(enDate)
    expect(zh).toContain(zhDate)
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

  test("renders a CSP-safe static loading state with its caller label", () => {
    const html = renderToStaticMarkup(
      <SignalDetailView loading signal={null} />,
    )
    expect(html).toContain('role="status"')
    expect(html).toContain("Loading signal detail")
    expect(html).not.toContain(" style=")
    expect(html).not.toContain("<style")
  })

  test("removes a not-found detail row from the list projection", () => {
    const html = renderToStaticMarkup(
      <SignalsScreenView
        boardName="Default"
        filters={{}}
        list={state([signal()])}
        detail={state(null, "error", { status: 404 })}
        selectedSignalId="sig_1"
        online
        onRefresh={() => undefined}
        onFiltersChange={() => undefined}
        onSelectSignal={() => undefined}
      />,
    )
    expect(html).not.toContain("CLI friction")
    expect(html).toContain("Signal is no longer available")
  })
})
