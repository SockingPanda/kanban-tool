import type { FeatureRoute } from "./BoardFeatureRoute"

export type FeatureInvalidationSource = "event" | "boundary" | null

export function telemetryInvalidationSource(view: FeatureRoute["view"], type: string, eventKind: unknown): FeatureInvalidationSource {
  if (type === "recovery-complete" || type === "poll-complete" || type === "poll-boundary-complete" || type === "protocol-anomaly" || type === "isolation-anomaly") return "boundary"
  if (type !== "event-applied" || typeof eventKind !== "string") return null
  if (view === "signals") return eventKind === "signal.recorded" || eventKind === "signal.reviewed" ? "event" : null
  return eventKind === "label.created"
    || eventKind === "label.deleted"
    || eventKind === "label.ontology.action.created"
    || eventKind === "label.ontology.observation.recorded"
    || eventKind === "label.ontology.signal.reviewed"
    || eventKind === "task.label_proposal.proposed"
    || eventKind === "task.label_proposal.accepted"
    || eventKind === "task.label_proposal.rejected"
    ? "event"
    : null
}

export function telemetryInvalidatesFeature(view: FeatureRoute["view"], type: string, eventKind: unknown): boolean {
  return telemetryInvalidationSource(view, type, eventKind) !== null
}
