import { useEffect, useRef, useState } from "react"

export type ReadPhase = "idle" | "loading" | "refreshing" | "success" | "error"

export interface ReadState<T> {
  readonly phase: ReadPhase
  readonly data: T
  readonly error: unknown | null
  /** Request identity used to fence first-frame and late-response state. */
  readonly requestKey?: string
  /** Monotonic request generation for callers that reconcile selections. */
  readonly generation?: number
}

/** Keep a deep-linked selection until an authoritative successful list arrives. */
export function reconcileSelection<T extends { readonly id: string }>(selectedId: string | null, rows: readonly T[]): string | null {
  if (selectedId === null) return null
  return rows.some((row) => row.id === selectedId) ? selectedId : rows[0]?.id ?? null
}

function hasPriorData<T>(state: ReadState<T>): boolean {
  return state.phase === "success" || state.phase === "refreshing" || state.phase === "error"
}

/** Shared feature-local async read state with abort and stale-response fencing. */
export function useReadState<T>(
  enabled: boolean,
  request: ((signal: AbortSignal) => Promise<T>) | null,
  key: string,
  initial: ReadState<T>,
): ReadState<T> {
  const [state, setState] = useState<ReadState<T>>(initial)
  const keyRef = useRef(key)
  const generationRef = useRef(0)
  const keyChanged = state.requestKey !== key
  const generation = keyChanged ? generationRef.current + 1 : generationRef.current

  useEffect(() => {
    const requestGeneration = generationRef.current + 1
    generationRef.current = requestGeneration
    keyRef.current = key
    if (!enabled || request === null) {
      setState({ ...initial, requestKey: key, generation: requestGeneration })
      return
    }
    const controller = new AbortController()
    setState((previous) => {
      const priorData = hasPriorData(previous)
      return {
        phase: priorData ? "refreshing" : "loading",
        data: priorData ? previous.data : initial.data,
        error: null,
        requestKey: key,
        generation: requestGeneration,
      }
    })
    void request(controller.signal).then(
      (data) => {
        if (controller.signal.aborted || keyRef.current !== key || generationRef.current !== requestGeneration) return
        setState({ phase: "success", data, error: null, requestKey: key, generation: requestGeneration })
      },
      (error: unknown) => {
        if (controller.signal.aborted || keyRef.current !== key || generationRef.current !== requestGeneration) return
        setState((previous) => ({
          phase: "error",
          data: previous.data,
          error,
          requestKey: key,
          generation: requestGeneration,
        }))
      },
    )
    return () => controller.abort()
    // request is recreated by callers only when this stable key changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [enabled, key])

  if (keyChanged || state.generation !== generation) {
    return {
      phase: enabled && request !== null
        ? hasPriorData(state) ? "refreshing" : "loading"
        : "idle",
      data: hasPriorData(state) ? state.data : initial.data,
      error: null,
      requestKey: key,
      generation,
    }
  }
  return state
}
