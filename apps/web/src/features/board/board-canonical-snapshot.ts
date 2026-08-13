import { BoardReadError, type BoardReadModel } from "../../lib/api/board-read-model"

/**
 * The immutable raw board read owned by BoardLive.  The key is the fenced
 * route/runtime context, not a selector that a visible child may reinterpret.
 */
export interface BoardCanonicalSnapshot {
  readonly key: string
  readonly generation: number
  readonly model: BoardReadModel | null
  readonly loading: boolean
  readonly error: BoardReadError | null
  readonly stale: boolean
}

/** Match the existing surface/release callback convention used by BoardLive. */
export type BoardCanonicalSnapshotChange = (
  snapshot: BoardCanonicalSnapshot | undefined,
  releasedSnapshot?: BoardCanonicalSnapshot,
) => void

export type BoardCanonicalSnapshotRetryChange = (
  retry: (() => void) | undefined,
  releasedRetry?: () => void,
) => void

export function createBoardCanonicalSnapshot(
  key: string,
  generation: number,
  state: Omit<BoardCanonicalSnapshot, "key" | "generation">,
): BoardCanonicalSnapshot {
  return Object.freeze({ key, generation, ...state })
}

/** Convert projection/transport failures to the typed BoardLive read error. */
export function boardCanonicalSnapshotError(error: unknown): BoardReadError {
  if (error instanceof BoardReadError) return error
  return new BoardReadError("anomaly", "canonical board snapshot read failed", { cause: error })
}

/** Context and generation fences reject callbacks from an obsolete BoardLive read. */
export function isCurrentBoardCanonicalSnapshot(
  snapshot: BoardCanonicalSnapshot,
  activeKey: string,
  activeGeneration: number,
): boolean {
  return snapshot.key === activeKey && snapshot.generation === activeGeneration
}
