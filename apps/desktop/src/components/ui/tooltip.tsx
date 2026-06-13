import * as React from "react"

import { cn } from "@/lib/utils"

export function TooltipProvider({ children }: { children: React.ReactNode }) {
  return <>{children}</>
}

export function Tooltip({
  children,
  content,
}: {
  children: React.ReactElement<{ title?: string; "aria-label"?: string }>
  content: string
}) {
  return React.cloneElement(children, {
    title: children.props.title ?? content,
    "aria-label": children.props["aria-label"] ?? content,
  })
}

export function TooltipContent({ className, ...props }: React.HTMLAttributes<HTMLDivElement>) {
  return (
    <div
      className={cn("rounded-md border border-border bg-card px-2 py-1 text-xs text-card-foreground shadow-sm", className)}
      {...props}
    />
  )
}
