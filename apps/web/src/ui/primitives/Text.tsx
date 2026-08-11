import { createElement, type HTMLAttributes, type ReactNode } from "react"

import styles from "./primitives.module.css"
import { cx } from "./shared"

export type TextTone = "primary" | "secondary" | "tertiary" | "danger" | "success" | "warning"
export type TextSize = "body" | "supporting" | "title" | "pageTitle" | "code"

export type TextProps = Omit<HTMLAttributes<HTMLElement>, "children"> & {
  readonly as?: "p" | "span" | "strong" | "small" | "h1" | "h2" | "h3" | "code"
  readonly tone?: TextTone
  readonly size?: TextSize
  readonly children?: ReactNode
}

export function Text({ as = "p", tone = "primary", size = "body", className, children, ...props }: TextProps) {
  return createElement(as, {
    ...props,
    className: cx(styles.text, styles[`text-${tone}`], styles[`text-${size}`], className),
    children,
  })
}
