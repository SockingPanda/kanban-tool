import type { ButtonHTMLAttributes, ReactNode } from "react"

import styles from "./primitives.module.css"
import { cx } from "./shared"

export type ButtonVariant = "primary" | "secondary" | "ghost" | "danger"
export type ControlSize = "sm" | "md"

export type ButtonProps = Omit<ButtonHTMLAttributes<HTMLButtonElement>, "children"> & {
  readonly variant?: ButtonVariant
  readonly size?: ControlSize
  readonly isLoading?: boolean
  readonly isDisabled?: boolean
  readonly children?: ReactNode
}

export function Button({ variant = "secondary", size = "md", isLoading = false, isDisabled = false, className, children, disabled, ...props }: ButtonProps) {
  return (
    <button
      {...props}
      className={cx(styles.button, styles[`button-${variant}`], styles[`control-${size}`], className)}
      disabled={disabled || isDisabled || isLoading}
      aria-busy={isLoading || undefined}
    >
      {isLoading ? <span className={styles.spinner} aria-hidden="true" /> : null}
      <span>{children}</span>
    </button>
  )
}
