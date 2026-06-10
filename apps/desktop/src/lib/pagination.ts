import type { PageMeta } from "@/lib/api"

export function hasNextPage(page: PageMeta, visibleTaskCount: number) {
  if (page.total === null) return visibleTaskCount === page.limit
  return page.offset + visibleTaskCount < page.total
}

export function pageRangeLabel(page: PageMeta, visibleTaskCount: number) {
  const start = visibleTaskCount ? page.offset + 1 : 0
  const end = page.offset + visibleTaskCount
  if (page.total === null) return `showing ${start}-${end} of at least ${end}`
  return `showing ${start}-${end} of ${page.total}`
}
