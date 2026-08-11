import type { HTMLAttributes, ReactNode } from "react"

import styles from "./Frame.module.css"

export type Density = "compact" | "comfortable"

export type FoundationFrameProps = Omit<HTMLAttributes<HTMLDivElement>, "children"> & {
  readonly density?: Density
  readonly children?: ReactNode
}

/**
 * A small, app-owned scope for stories and feature compositions.
 * It carries density without creating a second theme or persistence path.
 */
export function FoundationFrame({ density = "comfortable", className, children, ...props }: FoundationFrameProps) {
  return (
    <div {...props} className={[styles.frame, className].filter(Boolean).join(" ")} data-kb-density={density}>
      {children}
    </div>
  )
}
