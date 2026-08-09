import { ExplorerReadError } from "../../lib/api/explorer-read-model"
import type { BoardRouteView } from "../../lib/router"

export type AsyncReadState<T> = {
  readonly data: T | null
  readonly error: ExplorerReadError | Error | null
  readonly loading: boolean
}

export type AsyncReadToken = {
  readonly identityKey: string
  readonly requestKey: string
}

export type AsyncReadInternalState<T> = AsyncReadState<T> & AsyncReadToken

export function asyncReadToken(enabled: boolean, key: string, generation: number): AsyncReadToken {
  const identityKey = `${enabled ? "enabled" : "disabled"}|${key}`
  return { identityKey, requestKey: `${identityKey}|${generation}` }
}

export function visibleAsyncReadState<T>(
  state: AsyncReadInternalState<T>,
  token: AsyncReadToken,
  enabled: boolean,
): AsyncReadState<T> {
  const sameIdentity = state.identityKey === token.identityKey
  const currentRequest = sameIdentity && state.requestKey === token.requestKey
  return {
    data: sameIdentity ? state.data : null,
    error: currentRequest ? state.error : null,
    loading: sameIdentity ? (currentRequest ? state.loading : true) : enabled,
  }
}

export function shouldClearMapTaskFromInspector(
  view: BoardRouteView,
  taskId: string | null,
  error: ExplorerReadError | Error | null,
): boolean {
  return view === "map" && Boolean(taskId) && error instanceof ExplorerReadError && error.reason === "task-not-found"
}
