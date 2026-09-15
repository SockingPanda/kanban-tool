import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { ExplorerReadError } from '../data/explorer-read-model';
import { asyncReadToken, visibleAsyncReadState, type AsyncReadInternalState, type AsyncReadState } from './read-state';
export function useAsyncRead<T>(
  enabled: boolean,
  key: string,
  load: (signal: AbortSignal) => Promise<T>,
  refreshRevision = 0,
  online = true,
): AsyncReadState<T> & { readonly retry: () => void; readonly reload: () => Promise<T> } {
  const loadRef = useRef(load)
  useLayoutEffect(() => { loadRef.current = load });
  const reloadWaitersRef = useRef<Array<{ readonly identityKey: string; readonly minimumGeneration: number; readonly resolve: (data: T) => void; readonly reject: (error: unknown) => void }>>([])
  const settledRef = useRef<{ readonly identityKey: string; readonly requestKey: string; readonly generation: number; readonly data: T } | null>(null)
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

  // 写后回读只有在新数据已提交到 React 界面后才完成，避免释放 pending 时仍使用旧字段版本。
  useLayoutEffect(() => {
    const waiting = reloadWaitersRef.current.splice(0)
    const settled = settledRef.current
    for (const waiter of waiting) {
      if (waiter.identityKey !== identityKey || !enabled || !online) {
        waiter.reject(new ExplorerReadError("anomaly", "读取的项目或连接状态已改变。"))
      } else if (state.requestKey === requestKey && !state.loading && generation >= waiter.minimumGeneration) {
        if (state.error) waiter.reject(state.error)
        else if (settled?.requestKey === requestKey && settled.identityKey === identityKey && settled.generation >= waiter.minimumGeneration) waiter.resolve(settled.data)
        else reloadWaitersRef.current.push(waiter)
      } else reloadWaitersRef.current.push(waiter)
    }
  }, [enabled, generation, identityKey, online, requestKey, state])

  useEffect(() => {
    if (!enabled) {
      setState({ data: null, error: null, loading: false, identityKey, requestKey })
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
          settledRef.current = { data, identityKey, requestKey, generation }
          setState({ data, error: null, loading: false, identityKey, requestKey })
        }
      },
      (error: unknown) => {
        if (active) {
          setState((current) => ({
            data: current.identityKey === identityKey ? current.data : null,
            error: error instanceof Error ? error : new Error(String(error)),
            loading: false,
            identityKey,
            requestKey,
          }))
        }
      },
    )
    return () => {
      active = false
      controller.abort()
      // SSE 或显式刷新替换同项目请求时，等待者继续等待新请求；切换项目与卸载另行终结。
    }
  }, [enabled, generation, identityKey, key, online, requestKey])

  useEffect(() => () => {
    const pending = reloadWaitersRef.current.splice(0)
    for (const waiter of pending) waiter.reject(new ExplorerReadError("anomaly", "读取在当前 identity 下被卸载。"))
  }, [])

  const reload = useCallback(() => new Promise<T>((resolve, reject) => {
    reloadWaitersRef.current.push({ identityKey, minimumGeneration: generation + 1, resolve, reject })
    setGeneration((current) => current + 1)
  }), [generation, identityKey])

  return {
    ...visibleAsyncReadState(state, { identityKey, requestKey }, enabled),
    retry: () => setGeneration((current) => current + 1),
    reload,
  }
}
