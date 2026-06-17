import type { CommentRecord } from "@/lib/api"

export const COMMENT_PAGE_SIZE = 10

export type CommentSortOrder = "newest" | "oldest"

export type CommentPageState = {
  comments: CommentRecord[]
  page: number
  pageCount: number
  hasPreviousPage: boolean
  hasNextPage: boolean
  total: number
}

export function sortedComments(comments: CommentRecord[], sortOrder: CommentSortOrder): CommentRecord[] {
  return [...comments].sort((left, right) => {
    const createdDiff = left.created_at - right.created_at
    const fallbackDiff = left.id.localeCompare(right.id)
    const diff = createdDiff || fallbackDiff
    return sortOrder === "newest" ? -diff : diff
  })
}

export function commentPageState({
  comments,
  page,
  pageSize = COMMENT_PAGE_SIZE,
  sortOrder,
}: {
  comments: CommentRecord[]
  page: number
  pageSize?: number
  sortOrder: CommentSortOrder
}): CommentPageState {
  const total = comments.length
  const pageCount = Math.max(1, Math.ceil(total / pageSize))
  const currentPage = Math.min(Math.max(page, 0), pageCount - 1)
  const start = currentPage * pageSize
  const pageComments = sortedComments(comments, sortOrder).slice(start, start + pageSize)

  return {
    comments: pageComments,
    page: currentPage,
    pageCount,
    hasPreviousPage: currentPage > 0,
    hasNextPage: currentPage < pageCount - 1,
    total,
  }
}

