import type { LabelOntologyReviewGroup, LabelOntologySignalRecord } from "../../lib/api/signals-ontology-read-model"

/** The API contract caps ontology signal pages at 100 rows. */
export const ONTOLOGY_SIGNAL_SCOPE_LIMIT = 100

/**
 * Review groups have no board_id of their own. When the current board signal
 * page is shorter than its hard cap, it is complete and group signal_ids can
 * be checked without guessing a missing field. At the cap, unknown non-empty
 * IDs are ambiguous, so preserve those rows and rely on detail/list board
 * checks; empty signal_ids still fail closed because the service invariant
 * requires every review group to name at least one signal.
 */
export function scopeReviewGroupsToKnownSignals(
  groups: readonly LabelOntologyReviewGroup[],
  signals: readonly LabelOntologySignalRecord[],
  signalPageComplete: boolean,
): readonly LabelOntologyReviewGroup[] {
  const nonEmptyGroups = groups.filter((group) => group.signal_ids.length > 0)
  if (!signalPageComplete) return nonEmptyGroups
  const knownSignalIds = new Set(signals.map((signal) => signal.id))
  return nonEmptyGroups.filter((group) => group.signal_ids.every((signalId) => knownSignalIds.has(signalId)))
}
