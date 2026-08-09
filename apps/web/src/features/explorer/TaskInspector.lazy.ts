export interface InspectorAsyncFence {
  begin(identity: string): AbortSignal
  isCurrent(identity: string, signal: AbortSignal): boolean
  abort(): void
}

/** Each lazy section owns one fence so a late result cannot cross task/session identity. */
export function createInspectorAsyncFence(): InspectorAsyncFence {
  let currentIdentity = ""
  let controller = new AbortController()
  return {
    begin(identity) {
      controller.abort()
      controller = new AbortController()
      currentIdentity = identity
      return controller.signal
    },
    isCurrent(identity, signal) {
      return currentIdentity === identity && signal === controller.signal && !signal.aborted
    },
    abort() {
      controller.abort()
      currentIdentity = ""
    },
  }
}
