import { useId, type InputHTMLAttributes, type ReactNode } from "react"

import styles from "./primitives.module.css"
import { cx } from "./shared"

export type TextFieldProps = Omit<InputHTMLAttributes<HTMLInputElement>, "children"> & {
  readonly label: ReactNode
  readonly hint?: ReactNode
  readonly error?: ReactNode
  readonly startAdornment?: ReactNode
  readonly endAdornment?: ReactNode
  readonly isLoading?: boolean
}

export function TextField({ label, hint, error, startAdornment, endAdornment, isLoading = false, id, className, disabled, ...props }: TextFieldProps) {
  const generatedId = useId()
  const inputId = id ?? `kb-field-${generatedId}`
  const hintId = hint ? `${inputId}-hint` : undefined
  const errorId = error ? `${inputId}-error` : undefined
  const describedBy = [hintId, errorId, props["aria-describedby"]].filter(Boolean).join(" ") || undefined

  return (
    <div className={cx(styles.field, className)}>
      <label className={styles.fieldLabel} htmlFor={inputId}>{label}</label>
      <div className={cx(styles.inputFrame, error ? styles.inputError : undefined, disabled ? styles.inputDisabled : undefined)}>
        {startAdornment ? <span className={styles.adornment} aria-hidden="true">{startAdornment}</span> : null}
        <input
          {...props}
          id={inputId}
          className={styles.input}
          disabled={disabled || isLoading}
          aria-invalid={error ? true : props["aria-invalid"]}
          aria-describedby={describedBy}
          aria-busy={isLoading || undefined}
        />
        {isLoading ? <span className={styles.spinner} aria-hidden="true" /> : endAdornment ? <span className={styles.adornment} aria-hidden="true">{endAdornment}</span> : null}
      </div>
      {error ? <p className={styles.fieldError} id={errorId} role="alert">{error}</p> : hint ? <p className={styles.fieldHint} id={hintId}>{hint}</p> : null}
    </div>
  )
}
