import { createElement, type HTMLAttributes, type ReactNode } from "react"

import styles from "./primitives.module.css"
import { cx } from "./shared"

export type SurfaceTone = "canvas" | "surface" | "layer" | "card" | "popover"
export type SurfacePadding = "none" | "compact" | "comfortable"

export type SurfaceProps = Omit<HTMLAttributes<HTMLElement>, "children"> & {
  readonly as?: "div" | "main" | "section" | "article"
  readonly tone?: SurfaceTone
  readonly padding?: SurfacePadding
  readonly children?: ReactNode
}

export function Surface({ as = "div", tone = "surface", padding = "none", className, children, ...props }: SurfaceProps) {
  return createElement(as, {
    ...props,
    className: cx(styles.surface, styles[`tone-${tone}`], styles[`padding-${padding}`], className),
    children,
  })
}
