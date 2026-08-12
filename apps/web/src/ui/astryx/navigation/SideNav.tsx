import { useState } from "react"

import { joinClassNames } from "./classNames"
import { SideNavContext } from "./context"
import type { SideNavCollapseConfig, SideNavProps } from "./types"

const navClasses = {
  root: "flex min-h-0 w-64 flex-col overflow-hidden border-r border-border bg-surface text-primary",
  rootCollapsed: "w-16",
  topContent: "shrink-0 px-2 py-2",
  list: "m-0 flex min-h-0 flex-1 list-none flex-col gap-1 overflow-y-auto p-2",
  footer: "mt-auto shrink-0 border-t border-border px-2 py-2",
  footerList: "m-0 flex list-none flex-col gap-1 p-0",
  footerIcons: "mt-2 flex items-center justify-center gap-1",
  collapseButton: "w-full rounded-md border-0 bg-transparent px-3 py-2 text-left text-sm text-secondary outline-none hover:bg-overlay-hover focus-visible:outline-2 focus-visible:outline-accent",
} as const

function collapseConfig(value: SideNavProps["collapsible"]): SideNavCollapseConfig {
  if (value === true) return {}
  if (value === false || value === undefined) return {}
  return value
}

export function SideNav({
  children,
  header,
  topContent,
  footer,
  footerIcons,
  collapsible = false,
  className,
  "aria-label": ariaLabel,
  "aria-labelledby": ariaLabelledBy,
  "data-testid": testId,
}: SideNavProps) {
  const config = collapseConfig(collapsible)
  const isCollapsible = collapsible !== false
  const controlledCollapsed = config.isCollapsed
  const [localCollapsed, setLocalCollapsed] = useState(config.defaultIsCollapsed ?? false)
  const isCollapsed = controlledCollapsed ?? localCollapsed
  const hasCollapseButton = isCollapsible && config.hasButton !== false && config.buttonLabel !== undefined
  const hasFooter = footer !== undefined || footerIcons !== undefined || hasCollapseButton

  const setCollapsed = (next: boolean) => {
    if (controlledCollapsed === undefined) setLocalCollapsed(next)
    config.onCollapsedChange?.(next)
  }

  return (
    <SideNavContext.Provider value={{ isCollapsed }}>
      <nav
        className={joinClassNames(navClasses.root, isCollapsed && navClasses.rootCollapsed, className)}
        aria-label={ariaLabel}
        aria-labelledby={ariaLabelledBy}
        data-testid={testId}
        data-collapsed={isCollapsed ? "true" : "false"}
      >
        {header}
        {topContent !== undefined ? <section className={navClasses.topContent}>{topContent}</section> : null}
        <ul className={navClasses.list}>{children}</ul>
        {hasFooter ? (
          <footer className={navClasses.footer}>
            {footer !== undefined ? <ul className={navClasses.footerList}>{footer}</ul> : null}
            {hasCollapseButton ? (
              <button className={navClasses.collapseButton} type="button" aria-expanded={!isCollapsed} onClick={() => setCollapsed(!isCollapsed)}>
                {config.buttonLabel}
              </button>
            ) : null}
            {footerIcons !== undefined ? <menu className={navClasses.footerIcons}>{footerIcons}</menu> : null}
          </footer>
        ) : null}
      </nav>
    </SideNavContext.Provider>
  )
}

export default SideNav
