import { renderToStaticMarkup } from "react-dom/server"
import { QueryClient, QueryClientProvider } from "@tanstack/react-query"
import { describe, expect, it } from "vitest"

import type { SignalRecord } from "@/lib/api"

import { SignalDetail, SignalList, SignalsWorkbench } from "./SignalsWorkbench"

describe("SignalsWorkbench presentation", () => {
  it("renders the generic signal shell separate from ontology review", () => {
    const html = renderWorkbenchShell()

    expect(html).toContain("Signals")
    expect(html).toContain("Generic agent and product signals for the active board.")
    expect(html).toContain("Signal rows")
    expect(html).toContain("Signal detail")
    expect(html).toContain("Open + confirmed")
  })

  it("renders signal rows with generic status, kind, and task context", () => {
    const html = renderToStaticMarkup(
      <SignalList
        loading={false}
        signals={[
          signalFixture({ id: "sig_open", status: "open", kind: "agent_cli_friction" }),
          signalFixture({ id: "sig_confirmed", status: "confirmed", kind: "agent_workflow_violation" }),
        ]}
        selectedSignalId="sig_confirmed"
        onSelectSignal={() => undefined}
      />,
    )
    expect(html).toContain("open")
    expect(html).toContain("confirmed")
    expect(html).toContain("agent_cli_friction")
    expect(html).toContain("kanban-tool#371")
  })

  it("renders signal detail observation fields and evidence JSON", () => {
    const html = renderToStaticMarkup(
      <SignalDetail
        loading={false}
        signal={signalFixture({
          id: "sig_detail",
          title: "Argument mismatch",
          summary: "Agent selected a familiar flag form that the CLI rejected.",
          observation: {
            ...signalFixture().observation,
            actor: "codex",
            agent_type: "executor",
            source: "codex_cli_failure",
            evidence_json: "{\"command\":\"kanban task create\",\"exit_code\":1}",
          },
        })}
      />,
    )

    expect(html).toContain("Argument mismatch")
    expect(html).toContain("Agent selected a familiar flag form")
    expect(html).toContain("codex_cli_failure")
    expect(html).toContain("executor")
    expect(html).toContain("kanban task create")
  })
})

function renderWorkbenchShell(): string {
  const queryClient = new QueryClient()
  return renderToStaticMarkup(
    <QueryClientProvider client={queryClient}>
      <SignalsWorkbench api={null} />
    </QueryClientProvider>,
  )
}

function signalFixture(overrides: Partial<SignalRecord> = {}): SignalRecord {
  return {
    id: "sig_open",
    board_id: "b_1",
    observation_id: "obs_1",
    kind: "agent_cli_friction",
    title: "CLI friction",
    summary: "Agent observed a CLI mismatch.",
    severity: "info",
    status: "open",
    dedupe_key: "dedupe-cli",
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
      task_ref_snapshot: "kanban-tool#371",
      run_id: null,
      comment_id: null,
      actor: "codex",
      agent_type: "codex",
      source: "api-test",
      evidence_json: "{}",
      created_at: 1,
    },
    ...overrides,
  }
}
