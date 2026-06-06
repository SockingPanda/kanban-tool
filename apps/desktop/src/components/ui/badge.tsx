import * as React from "react"
import { cva, type VariantProps } from "class-variance-authority"

import { cn } from "@/lib/utils"

const badgeVariants = cva(
  "inline-flex items-center rounded-md px-2 py-0.5 text-xs font-medium ring-1 ring-inset",
  {
    variants: {
      variant: {
        default: "bg-neutral-900 text-white ring-neutral-900",
        secondary: "bg-neutral-100 text-neutral-700 ring-neutral-200",
        ready: "bg-emerald-50 text-emerald-700 ring-emerald-200",
        running: "bg-sky-50 text-sky-700 ring-sky-200",
        blocked: "bg-red-50 text-red-700 ring-red-200",
        review: "bg-amber-50 text-amber-800 ring-amber-200",
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
