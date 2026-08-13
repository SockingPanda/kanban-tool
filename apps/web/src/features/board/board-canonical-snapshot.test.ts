import { describe, expect, test } from "vitest"

import {
  createBoardCanonicalSnapshot,
  isCurrentBoardCanonicalSnapshot,
} from "./board-canonical-snapshot"

describe("BoardLive canonical snapshot fence", () => {
  test("publishes an immutable typed snapshot only for the active context generation", () => {
    const snapshot = createBoardCanonicalSnapshot("runtime\u0000default\u0000board", 3, {
      model: null,
      loading: true,
      error: null,
      stale: false,
    })

    expect(Object.isFrozen(snapshot)).toBe(true)
    expect(isCurrentBoardCanonicalSnapshot(snapshot, snapshot.key, 3)).toBe(true)
    expect(isCurrentBoardCanonicalSnapshot(snapshot, snapshot.key, 2)).toBe(false)
    expect(isCurrentBoardCanonicalSnapshot(snapshot, "runtime\u0000other\u0000board", 3)).toBe(false)
  })
})
