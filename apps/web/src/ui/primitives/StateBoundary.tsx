import type { ReactNode } from "react"

import { Button } from "./Button"
import styles from "./primitives.module.css"
import { cx } from "./shared"

export type BoundaryMode = "loading" | "empty" | "offline" | "stale" | "error"

export type StateBoundaryProps = {
  readonly mode: BoundaryMode
  readonly title: ReactNode
  readonly description?: ReactNode
  readonly action?: ReactNode
  readonly retryLabel?: ReactNode
  readonly onRetry?: () => void
  readonly className?: string
}

export function StateBoundary({ mode, title, description, action, retryLabel = "重试", onRetry, className }: StateBoundaryProps) {
  const isError = mode === "error"
  const role = isError ? "alert" : "status"

  return (
    <section className={cx(styles.stateBoundary, styles[`state-${mode}`], className)} role={role} aria-busy={mode === "loading" || undefined}>
      {mode === "loading" ? <span className={styles.boundarySpinner} aria-hidden="true" /> : <span className={styles.stateMarker} aria-hidden="true" />}
      <div className={styles.stateContent}>
        <h2 className={styles.stateTitle}>{title}</h2>
        {description ? <p className={styles.stateDescription}>{description}</p> : null}
        {action ? <div className={styles.stateAction}>{action}</div> : onRetry && mode !== "loading" ? <Button size="sm" variant={isError ? "danger" : "secondary"} onClick={onRetry}>{retryLabel}</Button> : null}
      </div>
    </section>
  )
}
