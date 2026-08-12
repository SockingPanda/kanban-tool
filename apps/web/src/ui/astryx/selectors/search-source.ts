import type { ReactNode } from "react"

export interface SearchableItem<TAuxiliary = unknown> {
  readonly id: string
  readonly label: string
  readonly element?: ReactNode
  readonly auxiliaryData?: TAuxiliary
}

export interface SearchSource<T extends SearchableItem = SearchableItem> {
  search(query: string): Promise<T[]> | T[]
  bootstrap(): Promise<T[]> | T[]
  cancel?(): void
}

export interface CreateStaticSourceOptions<T extends SearchableItem = SearchableItem> {
  readonly keywords?: (item: T) => readonly string[]
}

export function createStaticSource<T extends SearchableItem>(items: readonly T[], options?: CreateStaticSourceOptions<T> | ((item: T) => readonly string[])): SearchSource<T> {
  const keywords = typeof options === "function" ? options : options?.keywords
  return {
    search(query) {
      const normalized = query.trim().toLocaleLowerCase()
      if (normalized.length === 0) return [...items]
      return items.filter((item) => item.label.toLocaleLowerCase().includes(normalized) || (keywords?.(item) ?? []).some((word) => word.toLocaleLowerCase().includes(normalized)))
    },
    bootstrap() {
      return [...items]
    },
  }
}
