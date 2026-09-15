import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { ExplorerReadError } from '../data/explorer-read-model';
import { asyncReadToken, visibleAsyncReadState, type AsyncReadInternalState, type AsyncReadState } from './read-state';
export function useAsyncRead<T>(
  enabled: boolean,
  key: string,
  load: (signal: AbortSignal) => Promise<T>,
  refreshRevision = 0,
  online = true,
): AsyncReadState<T> & { readonly retry: () => void; readonly reload: () => Promise<void> } {
  const loadRef = useRef(load)
  useLayoutEffect(() => { loadRef.current = load });
  const reloadWaitersRef = useRef<Array<{ readonly resolve: () => void; readonly reject: (error: unknown) => void }>>([])
  const [generation, setGeneration] = useState(0)
  const { identityKey, requestKey: baseRequestKey } = asyncReadToken(enabled, key, generation)
  // A session event/poll boundary is a new request for the same visible
  // identity. Keep the last usable data while the coalesced refresh is in flight.
  const requestKey = `${baseRequestKey}\u0000${refreshRevision}`
  const [state, setState] = useState<AsyncReadInternalState<T>>(() => ({
    data: null,
    error: null,
    loading: false,
    identityKey,
    requestKey,
  }))

  useEffect(() => {
    const reloadWaiters = reloadWaitersRef.current.splice(0)
    const resolveReload = () => {
      for (const waiter of reloadWaiters) waiter.resolve()
    }
    const rejectReload = (error: unknown) => {
      for (const waiter of reloadWaiters) waiter.reject(error)
    }
    if (!enabled) {
      setState({ data: null, error: null, loading: false, identityKey, requestKey })
      rejectReload(new ExplorerReadError("anomaly", "当前读取未启用。"))
      return
    }
    if (!online) {
      setState((current) => ({
        data: current.identityKey === identityKey ? current.data : null,
        error: new ExplorerReadError("offline", "当前离线，无法加载 Explorer 数据。"),
        loading: false,
        identityKey,
        requestKey,
      }))
      rejectReload(new ExplorerReadError("offline", "当前离线，无法加载 Explorer 数据。"))
      return
    }
    const controller = new AbortController()
    let active = true
    setState((current) => ({
      data: current.identityKey === identityKey ? current.data : null,
      error: null,
      loading: true,
      identityKey,
      requestKey,
    }))
    void loadRef.current(controller.signal).then(
      (data) => {
        if (active) {
          setState({ data, error: null, loading: false, identityKey, requestKey })
          resolveReload()
        }
      },
      (error: unknown) => {
        if (active && error instanceof Error && error.name === "AbortError") {
          rejectReload(error)
        } else if (active) {
          setState((current) => ({
            data: current.identityKey === identityKey ? current.data : null,
            error: error instanceof Error ? error : new Error(String(error)),
            loading: false,
            identityKey,
            requestKey,
          }))
          rejectReload(error)
        }
      },
    )
    return () => {
      active = false
      controller.abort()
      rejectReload(new ExplorerReadError("anomaly", "读取在当前 identity 下被取消。"))
    }
  }, [enabled, generation, identityKey, key, online, requestKey])

  useEffect(() => () => {
    const pending = reloadWaitersRef.current.splice(0)
    for (const waiter of pending) waiter.reject(new ExplorerReadError("anomaly", "读取在当前 identity 下被卸载。"))
  }, [])

  const reload = useCallback(() => new Promise<void>((resolve, reject) => {
    reloadWaitersRef.current.push({ resolve, reject })
    setGeneration((current) => current + 1)
  }), [])

  return {
    ...visibleAsyncReadState(state, { identityKey, requestKey }, enabled),
    retry: () => setGeneration((current) => current + 1),
    reload,
  }
}
