import * as React from "react"

import { cn } from "@/lib/utils"

export const NativeSelect = React.forwardRef<HTMLSelectElement, React.SelectHTMLAttributes<HTMLSelectElement>>(
  function NativeSelect({ className, ...props }, ref) {
    return (
      <select
        ref={ref}
        className={cn(
          "h-8 rounded-md border border-border bg-input px-2 text-sm text-foreground outline-none transition-colors focus:border-ring",
          className,
        )}
        {...props}
      />
    )
  },
)
