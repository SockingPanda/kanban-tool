export const DESCRIPTION_COLLAPSE_LIMIT = 280

export function isLongDescription(description: string | null | undefined): boolean {
  return (description?.trim().length ?? 0) > DESCRIPTION_COLLAPSE_LIMIT
}

export function visibleDescription(description: string | null | undefined, expanded: boolean): string {
  const normalized = description?.trim() || "No description yet."
  if (expanded || normalized.length <= DESCRIPTION_COLLAPSE_LIMIT) return normalized
  return `${normalized.slice(0, DESCRIPTION_COLLAPSE_LIMIT).trimEnd()}...`
}
