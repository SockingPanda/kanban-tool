import { describe, expect, test } from "vitest"

import { telemetryInvalidatesFeature } from "./board-feature-invalidation"

describe("Board feature session invalidation", () => {
  test("keeps signal and ontology event ownership exact", () => {
    expect(telemetryInvalidatesFeature("signals", "event-applied", "signal.recorded")).toBe(true)
    expect(telemetryInvalidatesFeature("signals", "event-applied", "task.updated")).toBe(false)
    expect(telemetryInvalidatesFeature("ontology", "event-applied", "task.label_proposal.accepted")).toBe(true)
    expect(telemetryInvalidatesFeature("ontology", "event-applied", "label.ontology.action.created")).toBe(true)
    expect(telemetryInvalidatesFeature("ontology", "event-applied", "label.ontology.observation.recorded")).toBe(true)
    expect(telemetryInvalidatesFeature("ontology", "event-applied", "label.ontology.signal.reviewed")).toBe(true)
    expect(telemetryInvalidatesFeature("ontology", "event-applied", "signal.recorded")).toBe(false)
  })

  test("broadly refreshes after recovery or polling boundaries", () => {
    expect(telemetryInvalidatesFeature("signals", "recovery-complete", undefined)).toBe(true)
    expect(telemetryInvalidatesFeature("ontology", "poll-boundary-complete", undefined)).toBe(true)
    expect(telemetryInvalidatesFeature("signals", "protocol-anomaly", undefined)).toBe(true)
  })
})
