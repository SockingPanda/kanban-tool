import type { CSSProperties } from "react"

const BOARD_COLUMN_MIN_WIDTH_REM = 18
const DEFAULT_REM_PX = 16

export const boardScrollerClassName = "kb-native-scrollbar-fade min-h-0 min-w-0 flex-1 overflow-x-auto overflow-y-hidden bg-border"

export function boardGridStyle(columnCount: number): CSSProperties {
  const trackCount = Math.max(1, columnCount)

  return {
    gridTemplateColumns: `repeat(${trackCount}, minmax(${BOARD_COLUMN_MIN_WIDTH_REM}rem, 1fr))`,
    minWidth: `max(100%, ${trackCount * BOARD_COLUMN_MIN_WIDTH_REM}rem)`,
  }
}

export type BoardScroller = {
  clientWidth: number
  scrollLeft: number
}

export function boardMaxScrollLeft(columnCount: number, clientWidth: number, remPx = DEFAULT_REM_PX): number {
  const trackCount = Math.max(1, columnCount)
  const gridWidth = trackCount * BOARD_COLUMN_MIN_WIDTH_REM * remPx

  return Math.max(0, gridWidth - clientWidth)
}

export function clampBoardScrollLeft(scroller: BoardScroller, columnCount: number, remPx = DEFAULT_REM_PX): void {
  const maxScrollLeft = boardMaxScrollLeft(columnCount, scroller.clientWidth, remPx)

  if (scroller.scrollLeft > maxScrollLeft) {
    scroller.scrollLeft = maxScrollLeft
  } else if (scroller.scrollLeft < 0) {
    scroller.scrollLeft = 0
  }
}
