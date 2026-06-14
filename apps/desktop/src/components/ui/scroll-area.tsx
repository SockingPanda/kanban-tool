import * as React from "react"
import * as ScrollAreaPrimitive from "@radix-ui/react-scroll-area"

import { cn } from "@/lib/utils"

type ScrollAreaProps = React.ComponentPropsWithoutRef<typeof ScrollAreaPrimitive.Root> & {
  viewportClassName?: string
  viewportRef?: React.Ref<HTMLDivElement>
}

function setComposedRef<T>(ref: React.Ref<T> | undefined, value: T | null) {
  if (typeof ref === "function") {
    ref(value)
  } else if (ref) {
    ;(ref as React.MutableRefObject<T | null>).current = value
  }
}

export const ScrollArea = React.forwardRef<
  React.ComponentRef<typeof ScrollAreaPrimitive.Root>,
  ScrollAreaProps
>(function ScrollArea({ className, children, viewportClassName, viewportRef, type, ...props }, ref) {
  const rootRef = React.useRef<React.ComponentRef<typeof ScrollAreaPrimitive.Root> | null>(null)
  const scrollEndTimerRef = React.useRef<number | null>(null)

  React.useEffect(() => {
    return () => {
      if (scrollEndTimerRef.current) window.clearTimeout(scrollEndTimerRef.current)
    }
  }, [])

  function handleViewportScroll() {
    const root = rootRef.current
    if (!root) return

    root.dataset.scrolling = "true"
    if (scrollEndTimerRef.current) window.clearTimeout(scrollEndTimerRef.current)
    scrollEndTimerRef.current = window.setTimeout(() => {
      root.removeAttribute("data-scrolling")
      scrollEndTimerRef.current = null
    }, 700)
  }

  return (
    <ScrollAreaPrimitive.Root
      ref={(node) => {
        rootRef.current = node
        setComposedRef(ref, node)
      }}
      type={type ?? "always"}
      className={cn("relative min-h-0 overflow-hidden kb-scroll-area", className)}
      {...props}
    >
      <ScrollAreaPrimitive.Viewport
        ref={viewportRef}
        className={cn("h-full w-full rounded-[inherit]", viewportClassName)}
        onScroll={handleViewportScroll}
      >
        {children}
      </ScrollAreaPrimitive.Viewport>
      <ScrollBar />
      <ScrollBar orientation="horizontal" />
      <ScrollAreaPrimitive.Corner className="bg-muted" />
    </ScrollAreaPrimitive.Root>
  )
})

export const ScrollBar = React.forwardRef<
  React.ComponentRef<typeof ScrollAreaPrimitive.ScrollAreaScrollbar>,
  React.ComponentPropsWithoutRef<typeof ScrollAreaPrimitive.ScrollAreaScrollbar>
>(function ScrollBar({ className, orientation = "vertical", ...props }, ref) {
  return (
    <ScrollAreaPrimitive.ScrollAreaScrollbar
      ref={ref}
      orientation={orientation}
      className={cn(
        "kb-scroll-area__scrollbar flex touch-none select-none bg-transparent p-0.5 transition-colors",
        orientation === "vertical" && "h-full w-2.5 border-l border-l-transparent",
        orientation === "horizontal" && "h-2.5 flex-col border-t border-t-transparent",
        className,
      )}
      {...props}
    >
      <ScrollAreaPrimitive.ScrollAreaThumb className="kb-scroll-area__thumb relative flex-1 rounded-full bg-border hover:bg-ring" />
    </ScrollAreaPrimitive.ScrollAreaScrollbar>
  )
})
