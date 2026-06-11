import * as React from "react"

import { cn } from "@/lib/utils"

export const Textarea = React.forwardRef<HTMLTextAreaElement, React.TextareaHTMLAttributes<HTMLTextAreaElement>>(
  function Textarea({ className, ...props }, ref) {
    return (
      <textarea
        ref={ref}
        className={cn(
          "min-h-24 w-full resize-none rounded-md border border-neutral-200 bg-white px-3 py-2 text-sm outline-none transition-colors placeholder:text-neutral-400 focus:border-neutral-400",
          className,
        )}
        {...props}
      />
    )
  },
)
