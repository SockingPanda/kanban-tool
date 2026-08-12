import type { ReactNode } from "react"

export type NavigationIconName = "chevron-left" | "minus" | "plus"

type NavigationIconProps = {
  readonly name: NavigationIconName
  readonly size?: number
}

/** Local, dependency-free control icons for the CSP-safe navigation primitives. */
export function NavigationIcon({ name, size = 16 }: NavigationIconProps) {
  const common = {
    fill: "none",
    stroke: "currentColor",
    strokeLinecap: "round" as const,
    strokeLinejoin: "round" as const,
    strokeWidth: 1.5,
  }

  let path: ReactNode
  switch (name) {
    case "chevron-left":
      path = <path {...common} d="m10 3-5 5 5 5" />
      break
    case "minus":
      path = <path {...common} d="M3 8h10" />
      break
    case "plus":
      path = <path {...common} d="M8 3v10M3 8h10" />
      break
  }

  return (
    <svg
      aria-hidden="true"
      focusable="false"
      height={size}
      viewBox="0 0 16 16"
      width={size}
    >
      {path}
    </svg>
  )
}
