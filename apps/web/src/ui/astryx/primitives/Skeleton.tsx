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
  extends Omit<HTMLAttributes<HTMLElement>, "children" | "style">,
    NoRuntimeStyleProps {
  readonly size?: SkeletonSize
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
  const safeRest = guardNoRuntimeStyleProps(rest)
  return (
    <section
      ref={ref}
      aria-hidden="true"
      className={classNames(
        "motion-safe:animate-pulse bg-skeleton",
        SKELETON_SIZE_CLASSES[size],
        className,
      )}
      {...safeRest}
      data-size={size}
    />
  )
}

Skeleton.displayName = "CspSafeSkeleton"
