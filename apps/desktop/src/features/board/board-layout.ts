import type { CSSProperties } from "react"

const BOARD_COLUMN_MIN_WIDTH_REM = 18

export const boardScrollerClassName = "min-h-0 min-w-0 flex-1 overflow-x-auto overflow-y-hidden bg-border"

export function boardGridStyle(columnCount: number): CSSProperties {
  const trackCount = Math.max(1, columnCount)

  return {
    gridTemplateColumns: `repeat(${trackCount}, minmax(${BOARD_COLUMN_MIN_WIDTH_REM}rem, 1fr))`,
    minWidth: `max(100%, ${trackCount * BOARD_COLUMN_MIN_WIDTH_REM}rem)`,
  }
}
