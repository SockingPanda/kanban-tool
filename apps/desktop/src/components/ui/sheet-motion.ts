import { cn } from "@/lib/utils"

export type SheetSide = "top" | "right" | "bottom" | "left"

export function sheetOverlayClassName(className?: string) {
  return cn("kb-sheet-overlay fixed inset-0 z-50 bg-black/35", className)
}

export function sheetContentClassName(side: SheetSide) {
  return cn(
    "kb-sheet-content fixed z-50 flex flex-col gap-0 border-border bg-card text-card-foreground shadow-lg outline-none",
    `kb-sheet-content-${side}`,
  )
}
