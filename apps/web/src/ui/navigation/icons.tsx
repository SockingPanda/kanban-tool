import type { ReactNode } from "react"

export type NavigationIconName =
  | "activity"
  | "chevron-down"
  | "chevron-right"
  | "close"
  | "folder"
  | "grid"
  | "home"
  | "list"
  | "map"
  | "menu"
  | "search"
  | "settings"
  | "table"

type IconProps = {
  readonly name: NavigationIconName
  readonly size?: number
  readonly strokeWidth?: number
  readonly className?: string
  readonly children?: ReactNode
}

/** Small, dependency-free line icons for the navigation grammar. */
export function NavigationIcon({ name, size = 18, strokeWidth = 1.7, className, children }: IconProps) {
  const common = {
    fill: "none",
    stroke: "currentColor",
    strokeLinecap: "round" as const,
    strokeLinejoin: "round" as const,
    strokeWidth,
  }

  let path: ReactNode
  switch (name) {
    case "activity":
      path = <path {...common} d="M3 12h4l2.1-6 4.1 12 2.2-6H21" />
      break
    case "chevron-down":
      path = <path {...common} d="m6 9 6 6 6-6" />
      break
    case "chevron-right":
      path = <path {...common} d="m9 6 6 6-6 6" />
      break
    case "close":
      path = <path {...common} d="m6 6 12 12M18 6 6 18" />
      break
    case "folder":
      path = <path {...common} d="M3.5 6.5A1.5 1.5 0 0 1 5 5h5l2 2h7A1.5 1.5 0 0 1 20.5 8.5v8A1.5 1.5 0 0 1 19 18H5a1.5 1.5 0 0 1-1.5-1.5v-10Z" />
      break
    case "grid":
      path = <path {...common} d="M4 4h6v6H4V4Zm10 0h6v6h-6V4ZM4 14h6v6H4v-6Zm10 0h6v6h-6v-6Z" />
      break
    case "home":
      path = <path {...common} d="m3.5 10 8.5-7 8.5 7M5.5 9v10h13V9M9.5 19v-5h5v5" />
      break
    case "list":
      path = <path {...common} d="M8 6h12M8 12h12M8 18h12M4 6h.01M4 12h.01M4 18h.01" />
      break
    case "map":
      path = <path {...common} d="m3.5 5.5 5-2 7 3 5-2v14l-5 2-7-3-5 2v-14ZM8.5 3.5v14M15.5 6.5v14" />
      break
    case "menu":
      path = <path {...common} d="M4 7h16M4 12h16M4 17h16" />
      break
    case "search":
      path = <><circle {...common} cx="10.5" cy="10.5" r="6.5" /><path {...common} d="m16 16 4.5 4.5" /></>
      break
    case "settings":
      path = <><path {...common} d="m12 3 1.2 2.2 2.3.7 2.2-1.1 1.7 1.7-1.1 2.2.7 2.3L21 12l-2.1 1-.7 2.3 1.1 2.2-1.7 1.7-2.2-1.1-2.3.7L12 21l-1.1-2.2-2.3-.7-2.2 1.1-1.7-1.7 1.1-2.2L5.1 13 3 12l2.1-1 .7-2.3-1.1-2.2 1.7-1.7 2.2 1.1 2.3-.7L12 3Z" /><circle {...common} cx="12" cy="12" r="3" /></>
      break
    case "table":
      path = <path {...common} d="M4 4h16v16H4V4Zm0 5h16M4 15h16M10 9v11M16 9v11" />
      break
  }

  return (
    <svg
      aria-hidden="true"
      className={className}
      focusable="false"
      height={size}
      viewBox="0 0 24 24"
      width={size}
    >
      {path}
      {children}
    </svg>
  )
}
