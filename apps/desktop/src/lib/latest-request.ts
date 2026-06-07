export type LatestRequestGuard = {
  begin: () => LatestRequestToken
}

export type LatestRequestToken = {
  isLatest: () => boolean
}

export function createLatestRequestGuard(): LatestRequestGuard {
  let latestRequestId = 0

  return {
    begin() {
      latestRequestId += 1
      const requestId = latestRequestId
      return {
        isLatest: () => requestId === latestRequestId,
      }
    },
  }
}

export async function runLatestRequest<T>(
  guard: LatestRequestGuard,
  load: () => Promise<T>,
  commit: (result: T) => void,
  onLatestError?: (error: unknown) => void,
): Promise<boolean> {
  const request = guard.begin()

  try {
    const result = await load()
    if (!request.isLatest()) return false
    commit(result)
    return true
  } catch (error) {
    if (!request.isLatest()) return false
    onLatestError?.(error)
    throw error
  }
}
