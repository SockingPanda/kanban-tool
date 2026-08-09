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
  if (keyRef.current !== key) {
    keyRef.current = key
    generationRef.current += 1
  }
  const generation = generationRef.current

  useEffect(() => {
    if (!enabled || request === null) {
      setState({ ...initial, requestKey: key, generation })
      return
    }
    const controller = new AbortController()
    setState((previous) => {
      const sameRequest = previous.requestKey === key && previous.generation === generation
      return {
        phase: sameRequest && hasPriorData(previous) ? "refreshing" : "loading",
        data: sameRequest ? previous.data : initial.data,
        error: null,
        requestKey: key,
        generation,
      }
    })
    void request(controller.signal).then(
      (data) => {
        if (controller.signal.aborted || keyRef.current !== key || generationRef.current !== generation) return
        setState({ phase: "success", data, error: null, requestKey: key, generation })
      },
      (error: unknown) => {
        if (controller.signal.aborted || keyRef.current !== key || generationRef.current !== generation) return
        setState((previous) => ({
          phase: "error",
          data: previous.requestKey === key && previous.generation === generation ? previous.data : initial.data,
          error,
          requestKey: key,
          generation,
        }))
      },
    )
    return () => controller.abort()
    // request is recreated by callers only when this stable key changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [enabled, key])

  if (state.requestKey !== key || state.generation !== generation) {
    return {
      phase: enabled && request !== null ? "loading" : "idle",
      data: initial.data,
      error: null,
      requestKey: key,
      generation,
    }
  }
  return state
}
