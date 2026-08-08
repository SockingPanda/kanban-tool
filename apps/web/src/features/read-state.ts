import { useEffect, useState } from "react"

export type ReadPhase = "idle" | "loading" | "refreshing" | "success" | "error"

export interface ReadState<T> {
  readonly phase: ReadPhase
  readonly data: T
  readonly error: unknown | null
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

  useEffect(() => {
    if (!enabled || request === null) {
      setState(initial)
      return
    }
    const controller = new AbortController()
    setState((previous) => ({
      phase: hasPriorData(previous) ? "refreshing" : "loading",
      data: previous.data,
      error: null,
    }))
    void request(controller.signal).then(
      (data) => {
        if (controller.signal.aborted) return
        setState({ phase: "success", data, error: null })
      },
      (error: unknown) => {
        if (controller.signal.aborted) return
        setState((previous) => ({ phase: "error", data: previous.data, error }))
      },
    )
    return () => controller.abort()
    // request is recreated by callers only when this stable key changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [enabled, key])

  return state
}
