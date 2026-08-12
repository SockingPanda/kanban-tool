/* eslint-disable react-refresh/only-export-components */
import type { ReactNode } from "react"
import type { HTMLAttributes } from "react"

import type {NoRuntimeStyleProps} from "./safe-core"
import {pickCspSafeDomProps} from "../dom-props"

/**
 * Finite column contracts. The `auto-*` variants select viewport breakpoints;
 * they are not intrinsic-content or `auto-fit` sizing modes.
 */
export type GridColumns =
  | "single"
  | "two"
  | "three"
  | "auto-sm"
  | "auto-md"
  | "auto-lg"

export type GridGap = 0 | 1 | 2 | 3 | 4 | 5 | 6
export type GridDensity = "compact" | "comfortable"
export type GridAlign = "start" | "center" | "end" | "stretch"

/** Finite viewport variants map to literal classes; no runtime CSS is built. */
export const GRID_COLUMN_CLASSES: Readonly<Record<GridColumns, string>> = {
  single: "grid-cols-1",
  two: "grid-cols-2",
  three: "grid-cols-3",
  "auto-sm": "grid-cols-1 sm:grid-cols-2 lg:grid-cols-3",
  "auto-md": "grid-cols-1 md:grid-cols-2 xl:grid-cols-3",
  "auto-lg": "grid-cols-1 lg:grid-cols-2 2xl:grid-cols-3",
}

/** Gap is also finite; arbitrary values cannot enter this primitive. */
export const GRID_GAP_CLASSES: Readonly<Record<GridGap, string>> = {
  0: "gap-0",
  1: "gap-1",
  2: "gap-2",
  3: "gap-3",
  4: "gap-4",
  5: "gap-5",
  6: "gap-6",
}

export const GRID_DENSITY_CLASSES: Readonly<Record<GridDensity, string>> = {
  compact: "gap-2",
  comfortable: "gap-4",
}

export const GRID_ALIGN_CLASSES: Readonly<Record<GridAlign, string>> = {
  start: "items-start",
  center: "items-center",
  end: "items-end",
  stretch: "items-stretch",
}

export interface GridProps
  extends Omit<HTMLAttributes<HTMLElement>, "children" | "style">,
    NoRuntimeStyleProps {
  readonly children?: ReactNode
  /** Required accessible name because the primitive renders a section landmark. */
  readonly label: string
  readonly columns?: GridColumns
  readonly gap?: GridGap
  readonly density?: GridDensity
  readonly align?: GridAlign
  readonly ref?: React.Ref<HTMLElement>
}

function classNames(...values: Array<string | false | null | undefined>): string {
  return values.filter(Boolean).join(" ")
}

/**
 * CSP-safe grid facade.
 *
 * It keeps the useful Grid vocabulary while rendering only literal classes;
 * unlike the general Astryx Grid, it never computes a track string or emits a
 * CSS custom property at render time.
 */
export function Grid({
  align = "stretch",
  children,
  className,
  columns = "single",
  density,
  gap,
  id,
  label,
  ref,
  ...rest
}: GridProps) {
  const safeRest = pickCspSafeDomProps(rest)
  const gapClass = gap == null
    ? density == null
      ? "gap-3"
      : GRID_DENSITY_CLASSES[density]
    : GRID_GAP_CLASSES[gap]

  return (
    <section
      ref={ref}
      id={id}
      className={classNames(
        "grid min-w-0",
        GRID_COLUMN_CLASSES[columns],
        gapClass,
        GRID_ALIGN_CLASSES[align],
        className,
      )}
      {...safeRest}
      aria-label={label}
      data-columns={columns}
      data-density={density}
    >
      {children}
    </section>
  )
}

Grid.displayName = "CspSafeGrid"
