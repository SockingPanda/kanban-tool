import type { ReactNode } from "react"

import styles from "./primitives.module.css"
import { cx } from "./shared"

export type SegmentOption<Value extends string = string> = {
  readonly value: Value
  readonly label: ReactNode
  readonly disabled?: boolean
}

export type SegmentedControlProps<Value extends string = string> = {
  readonly label: string
  readonly options: readonly SegmentOption<Value>[]
  readonly value: Value
  readonly onChange?: (value: Value) => void
  readonly size?: ControlSize
  readonly className?: string
}

type ControlSize = "sm" | "md"

export function SegmentedControl<Value extends string>({ label, options, value, onChange, size = "md", className }: SegmentedControlProps<Value>) {
  return (
    <div className={cx(styles.segmented, styles[`control-${size}`], className)} role="group" aria-label={label}>
      {options.map((option) => (
        <button
          className={cx(styles.segment, option.value === value ? styles.segmentSelected : undefined)}
          key={option.value}
          type="button"
          disabled={option.disabled}
          aria-pressed={option.value === value}
          onClick={() => onChange?.(option.value)}
        >
          {option.label}
        </button>
      ))}
    </div>
  )
}
