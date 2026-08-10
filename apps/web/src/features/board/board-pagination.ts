export const BOARD_PAGE_SIZE = 100

export interface BoardPageWindow {
  readonly page: number
  readonly totalPages: number
  /** 用于切片 canonical 任务数组的零基包含起点。 */
  readonly start: number
  /** 用于切片 canonical 任务数组的零基排他终点。 */
  readonly end: number
}

/**
 * 将请求页夹取到 canonical 任务数量，不存储派生状态。
 * 因此即使 mutation 缩小列内容，render 仍可保留该列的页 identity。
 */
export function boardPageWindow(totalItems: number, requestedPage: number): BoardPageWindow {
  const normalizedTotal = Number.isFinite(totalItems) ? Math.max(0, Math.floor(totalItems)) : 0
  const totalPages = Math.max(1, Math.ceil(normalizedTotal / BOARD_PAGE_SIZE))
  const normalizedPage = Number.isFinite(requestedPage) ? Math.trunc(requestedPage) : 1
  const page = Math.min(totalPages, Math.max(1, normalizedPage))
  const start = normalizedTotal === 0 ? 0 : (page - 1) * BOARD_PAGE_SIZE
  const end = Math.min(start + BOARD_PAGE_SIZE, normalizedTotal)
  return { page, totalPages, start, end }
}
