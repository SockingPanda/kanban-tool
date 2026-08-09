import type { LabelOntologyReviewGroup, LabelOntologySignalRecord } from "../../lib/api/signals-ontology-read-model"

/** The API contract caps ontology signal pages at 100 rows. */
export const ONTOLOGY_SIGNAL_SCOPE_LIMIT = 100

/**
 * Review groups have no board_id of their own. When the current board signal
 * page is shorter than its hard cap, it is complete and group signal_ids can
 * be checked without guessing a missing field. At the cap, absence is
 * ambiguous, so preserve the response and rely on detail/list board checks.
 */
export function scopeReviewGroupsToKnownSignals(
  groups: readonly LabelOntologyReviewGroup[],
  signals: readonly LabelOntologySignalRecord[],
  signalPageComplete: boolean,
): readonly LabelOntologyReviewGroup[] {
  if (!signalPageComplete) return groups
  const knownSignalIds = new Set(signals.map((signal) => signal.id))
  return groups.filter((group) => group.signal_ids.every((signalId) => knownSignalIds.has(signalId)))
}
