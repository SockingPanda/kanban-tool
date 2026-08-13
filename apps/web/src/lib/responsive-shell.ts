import { useEffect, useState } from "react"

export type ShellViewportMode = "desktop" | "tablet" | "mobile"

export function shellViewportModeForWidth(width: number): ShellViewportMode {
  if (!Number.isFinite(width) || width < 768) return "mobile"
  if (width < 1024) return "tablet"
  return "desktop"
}

function currentViewportMode(): ShellViewportMode {
  return typeof window === "undefined"
    ? "desktop"
    : shellViewportModeForWidth(window.innerWidth)
}

/** The only browser viewport owner for shell mode decisions. */
export function useResponsiveShell(): ShellViewportMode {
  const [mode, setMode] = useState<ShellViewportMode>(currentViewportMode)

  useEffect(() => {
    if (typeof window === "undefined") return
    const update = () => setMode(currentViewportMode())
    update()
    window.addEventListener("resize", update)
    return () => window.removeEventListener("resize", update)
  }, [])

  return mode
}
