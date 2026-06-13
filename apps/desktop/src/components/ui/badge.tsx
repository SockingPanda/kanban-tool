import * as React from "react"
import { cva, type VariantProps } from "class-variance-authority"

import { cn } from "@/lib/utils"

const badgeVariants = cva(
  "inline-flex items-center rounded-md px-2 py-0.5 text-xs font-medium ring-1 ring-inset",
  {
    variants: {
      variant: {
        default: "bg-primary text-primary-foreground ring-primary",
        secondary: "bg-muted text-muted-foreground ring-border",
        ready: "bg-[var(--status-ready-bg)] text-[var(--status-ready-fg)] ring-[var(--status-ready-ring)]",
        running: "bg-[var(--status-running-bg)] text-[var(--status-running-fg)] ring-[var(--status-running-ring)]",
        blocked: "bg-[var(--status-blocked-bg)] text-[var(--status-blocked-fg)] ring-[var(--status-blocked-ring)]",
        review: "bg-[var(--status-review-bg)] text-[var(--status-review-fg)] ring-[var(--status-review-ring)]",
      },
    },
    defaultVariants: {
      variant: "secondary",
    },
  },
)

export interface BadgeProps
  extends React.HTMLAttributes<HTMLSpanElement>,
    VariantProps<typeof badgeVariants> {}

export function Badge({ className, variant, ...props }: BadgeProps) {
  return <span className={cn(badgeVariants({ variant, className }))} {...props} />
}
