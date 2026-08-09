import type { FeatureRoute } from "./BoardFeatureRoute"

export function telemetryInvalidatesFeature(view: FeatureRoute["view"], type: string, eventKind: unknown): boolean {
  if (type === "recovery-complete" || type === "poll-complete" || type === "poll-boundary-complete" || type === "protocol-anomaly" || type === "isolation-anomaly") return true
  if (type !== "event-applied" || typeof eventKind !== "string") return false
  if (view === "signals") return eventKind === "signal.recorded" || eventKind === "signal.reviewed"
  return eventKind === "label.created"
    || eventKind === "label.deleted"
    || eventKind === "label.ontology.action.created"
    || eventKind === "label.ontology.observation.recorded"
    || eventKind === "label.ontology.signal.reviewed"
    || eventKind === "task.label_proposal.proposed"
    || eventKind === "task.label_proposal.accepted"
    || eventKind === "task.label_proposal.rejected"
}
