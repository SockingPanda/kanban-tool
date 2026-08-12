/* eslint-disable react-refresh/only-export-components */
import type { HTMLAttributes } from "react"

import {
  guardNoRuntimeStyleProps,
  type NoRuntimeStyleProps,
} from "./safe-core"

export type SkeletonSize = "text" | "row" | "card"

export const SKELETON_SIZE_CLASSES: Readonly<Record<SkeletonSize, string>> = {
  text: "h-4 w-24 rounded-sm",
  row: "h-10 w-full rounded-sm",
  card: "h-32 w-full rounded-md",
}

export interface SkeletonProps
  extends Omit<HTMLAttributes<HTMLElement>, "children" | "style" | "aria-hidden">,
    NoRuntimeStyleProps {
  readonly size?: SkeletonSize
  /** Legacy stagger input is rejected and stripped for JS spread callers. */
  readonly index?: never
  /** Skeletons are always decorative and cannot opt into an exposed name. */
  readonly "aria-hidden"?: never
  readonly ref?: React.Ref<HTMLElement>
}

function classNames(...values: Array<string | false | null | undefined>): string {
  return values.filter(Boolean).join(" ")
}

/** Static geometry placeholder for strict-CSP loading states. */
export function Skeleton({
  className,
  size = "row",
  ref,
  ...rest
}: SkeletonProps) {
  const legacySafeRest = {...rest} as Record<string, unknown>
  delete legacySafeRest.index
  delete legacySafeRest["aria-hidden"]
  const safeRest = guardNoRuntimeStyleProps(legacySafeRest)
  return (
    <section
      ref={ref}
      className={classNames(
        "motion-safe:animate-pulse bg-skeleton",
        SKELETON_SIZE_CLASSES[size],
        className,
      )}
      {...safeRest}
      aria-hidden="true"
      data-size={size}
    />
  )
}

Skeleton.displayName = "CspSafeSkeleton"
