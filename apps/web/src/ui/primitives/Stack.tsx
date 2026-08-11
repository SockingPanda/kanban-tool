import type { HTMLAttributes, ReactNode } from "react"

import styles from "./primitives.module.css"
import { cx } from "./shared"

export type LayoutGap = "none" | "tight" | "default" | "loose"

export type StackProps = Omit<HTMLAttributes<HTMLDivElement>, "children"> & {
  readonly gap?: LayoutGap
  readonly children?: ReactNode
}

export function Stack({ gap = "default", className, children, ...props }: StackProps) {
  return <div {...props} className={cx(styles.stack, styles[`gap-${gap}`], className)}>{children}</div>
}

export type InlineProps = StackProps & {
  readonly align?: "start" | "center" | "end" | "stretch"
  readonly justify?: "start" | "center" | "end" | "between"
  readonly wrap?: boolean
}

export function Inline({ align = "center", justify = "start", wrap = true, gap = "default", className, children, ...props }: InlineProps) {
  return (
    <div
      {...props}
      className={cx(styles.inline, styles[`gap-${gap}`], styles[`align-${align}`], styles[`justify-${justify}`], wrap ? styles.wrap : styles.noWrap, className)}
    >
      {children}
    </div>
  )
}
