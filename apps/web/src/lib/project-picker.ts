import { parseCanonicalBoardSlug, type CanonicalBoardSlug } from "./board-slug"
import type { BoardListItem } from "./api/board-list-read-model"
import type { AppRoute } from "./router"

const RECENT_PROJECTS_STORAGE_KEY = "kb:web:recent-boards:v1"
const RECENT_PROJECTS_STORAGE_EVENT = "kb:web:recent-boards-changed"
const RECENT_PROJECTS_VERSION = 1
const MAX_RECENT_PROJECTS = 5

type RecentProjectsStorage = {
  readonly version: typeof RECENT_PROJECTS_VERSION
  readonly slugs: readonly string[]
}

function parseStoredRecentProjectSlugs(raw: string | null): readonly CanonicalBoardSlug[] {
  if (raw === null) return []

  let parsed: unknown
  try {
    parsed = JSON.parse(raw)
  } catch {
    return []
  }
  if (parsed === null || typeof parsed !== "object") return []
  const stored = parsed as Partial<RecentProjectsStorage>
  if (stored.version !== RECENT_PROJECTS_VERSION || !Array.isArray(stored.slugs)) return []

  const seen = new Set<string>()
  const slugs: CanonicalBoardSlug[] = []
  for (const value of stored.slugs) {
    const slug = parseCanonicalBoardSlug(value)
    if (slug === null || seen.has(slug)) continue
    seen.add(slug)
    slugs.push(slug)
    if (slugs.length >= MAX_RECENT_PROJECTS) break
  }
  return slugs
}

export function readRecentProjectSlugs(): readonly CanonicalBoardSlug[] {
  if (typeof window === "undefined") return []
  let raw: string | null
  try {
    raw = window.localStorage.getItem(RECENT_PROJECTS_STORAGE_KEY)
  } catch {
    return []
  }
  return parseStoredRecentProjectSlugs(raw)
}

export function parseRecentProjectSlugs(raw: string | null): readonly CanonicalBoardSlug[] {
  return parseStoredRecentProjectSlugs(raw)
}

export function rememberRecentProject(slug: CanonicalBoardSlug): readonly CanonicalBoardSlug[] {
  const next = [slug, ...readRecentProjectSlugs().filter((candidate) => candidate !== slug)].slice(0, MAX_RECENT_PROJECTS)
  if (typeof window === "undefined") return next
  const value: RecentProjectsStorage = { version: RECENT_PROJECTS_VERSION, slugs: next }
  try {
    window.localStorage.setItem(RECENT_PROJECTS_STORAGE_KEY, JSON.stringify(value))
    window.dispatchEvent(new Event(RECENT_PROJECTS_STORAGE_EVENT))
  } catch {
    // Private browsing and disabled storage should not make the picker unusable.
  }
  return next
}

/**
 * 只有 router 明确返回目标 canonical board route 后，项目才会进入 recent。
 * void、过期/当前 route、畸形对象或导航异常都不得写入 recent。
 */
export function isSuccessfulProjectNavigation(
  result: unknown,
  targetSlug: CanonicalBoardSlug,
  activeSlug?: CanonicalBoardSlug,
): result is Extract<AppRoute, { kind: "board" }> {
  if (activeSlug === targetSlug || typeof result !== "object" || result === null) return false
  const route = result as Partial<Extract<AppRoute, { kind: "board" }>>
  return route.kind === "board" && route.boardSlug === targetSlug && typeof route.pathname === "string"
}

export const recentProjectsStorageEvent = RECENT_PROJECTS_STORAGE_EVENT
export const recentProjectsStorageKey = RECENT_PROJECTS_STORAGE_KEY

function projectMatches(item: BoardListItem, query: string, locale: string): boolean {
  const normalizedQuery = query.trim().toLocaleLowerCase(locale)
  if (normalizedQuery.length === 0) return true
  return [item.name, item.slug]
    .some((value) => value.toLocaleLowerCase(locale).includes(normalizedQuery))
}

export type ProjectPickerGroups = {
  readonly current: readonly BoardListItem[]
  readonly recent: readonly BoardListItem[]
  readonly all: readonly BoardListItem[]
}

export function projectPickerGroups(
  items: readonly BoardListItem[],
  activeBoardSlug: CanonicalBoardSlug | undefined,
  recentSlugs: readonly CanonicalBoardSlug[],
  query: string,
  locale: string,
): ProjectPickerGroups {
  const matchingItems = items.filter((item) => projectMatches(item, query, locale))
  const current = matchingItems.filter((item) => item.slug === activeBoardSlug)
  const currentSlugs = new Set(current.map((item) => item.slug))
  const recentSlugSet = new Set(recentSlugs)
  const recent = recentSlugs
    .map((slug) => matchingItems.find((item) => item.slug === slug))
    .filter((item): item is BoardListItem => item !== undefined && !currentSlugs.has(item.slug))
  const recentSlugsInItems = new Set(recent.map((item) => item.slug))
  const all = matchingItems.filter((item) => !currentSlugs.has(item.slug) && !recentSlugsInItems.has(item.slug) && !recentSlugSet.has(item.slug))
  return { current, recent, all }
}
