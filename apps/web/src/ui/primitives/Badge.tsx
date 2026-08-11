import type { HTMLAttributes, ReactNode } from "react"

import styles from "./primitives.module.css"
import { cx } from "./shared"

export type BadgeTone = "neutral" | "accent" | "success" | "warning" | "danger"

export type BadgeProps = Omit<HTMLAttributes<HTMLSpanElement>, "children"> & {
  readonly tone?: BadgeTone
  readonly size?: "sm" | "md"
  readonly children?: ReactNode
}

export function Badge({ tone = "neutral", size = "sm", className, children, ...props }: BadgeProps) {
  return <span {...props} className={cx(styles.badge, styles[`badge-${tone}`], styles[`badge-${size}`], className)}>{children}</span>
}
