import { createContext, useContext } from "react"

export type SideNavContextValue = {
  readonly isCollapsed: boolean
}

const defaultSideNavContext: SideNavContextValue = { isCollapsed: false }

export const SideNavContext = createContext<SideNavContextValue>(defaultSideNavContext)

export function useSideNavContext(): SideNavContextValue {
  return useContext(SideNavContext)
}
