/**
 * Agent-first entry points are a view over canonical `tasks.status` values.
 * Keep this list deliberately small: it is a lens, not a second lifecycle.
 */
export const attentionLenses = ["ready", "running", "blocked", "review"] as const

export type AttentionLens = (typeof attentionLenses)[number]

export function isAttentionLens(value: string): value is AttentionLens {
  return attentionLenses.includes(value as AttentionLens)
}

export function activeAttentionLens(statuses: readonly string[]): AttentionLens | null {
  const candidate = statuses.length === 1 ? statuses[0] : undefined
  return candidate !== undefined && isAttentionLens(candidate) ? candidate : null
}

export function attentionStatusSelection<T extends string>(current: readonly T[], lens: T): readonly T[] {
  return current.length === 1 && current[0] === lens ? [] : [lens]
}

export function queryWithAttentionLens<T extends { readonly status: readonly string[]; readonly page: number }>(query: T, lens: AttentionLens): T {
  return { ...query, status: attentionStatusSelection(query.status, lens), page: 1 } as T
}

export function attentionCounts<T extends { readonly status: string }>(rows: readonly T[]): Readonly<Record<AttentionLens, number>> {
  const counts: Record<AttentionLens, number> = { ready: 0, running: 0, blocked: 0, review: 0 }
  for (const row of rows) {
    if (isAttentionLens(row.status)) counts[row.status] += 1
  }
  return counts
}

export function attentionTasks<T extends { readonly status: string }>(rows: readonly T[], lens: AttentionLens | null): readonly T[] {
  return lens === null ? rows : rows.filter((row) => row.status === lens)
}
