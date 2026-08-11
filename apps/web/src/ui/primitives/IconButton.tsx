import type { ButtonHTMLAttributes, ReactNode } from "react"

import styles from "./primitives.module.css"
import { cx } from "./shared"

export type IconButtonProps = Omit<ButtonHTMLAttributes<HTMLButtonElement>, "children"> & {
  readonly size?: ControlIconSize
  readonly children: ReactNode
}

export type ControlIconSize = "sm" | "md"

export function IconButton({ size = "md", className, children, ...props }: IconButtonProps) {
  return <button {...props} className={cx(styles.iconButton, styles[`icon-${size}`], className)}>{children}</button>
}
