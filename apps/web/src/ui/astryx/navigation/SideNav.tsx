import { useRef, useState, type Ref } from "react"

import { joinClassNames } from "./classNames"
import { SideNavContext } from "./context"
import type { SideNavProps } from "./types"

const navClasses = {
  root: "flex min-h-0 w-64 flex-col overflow-hidden border-e border-border bg-surface text-primary",
  rootCollapsed: "w-16",
  topContent: "shrink-0 px-2 py-2",
  list: "m-0 flex min-h-0 flex-1 list-none flex-col gap-1 overflow-y-auto p-2",
  footer: "mt-auto shrink-0 border-t border-border px-2 py-2",
  footerList: "m-0 flex list-none flex-col gap-1 p-0",
  footerIcons: "mt-2 flex items-center justify-center gap-1",
  collapseButton: "w-full rounded-md border-0 bg-transparent px-3 py-2 text-start text-sm text-secondary outline-none hover:bg-overlay-hover focus-visible:outline-2 focus-visible:outline-accent",
  collapseButtonCollapsed: "flex justify-center px-2",
} as const

function assignRef<T>(ref: Ref<T> | undefined | null, value: T | null): void {
  if (typeof ref === "function") ref(value)
  else if (ref !== undefined && ref !== null) ref.current = value
}

export function SideNav({
  children,
  header,
  topContent,
  footer,
  footerIcons,
  collapsible,
  id,
  ref,
  className,
  "aria-label": ariaLabel,
  "aria-labelledby": ariaLabelledBy,
  "data-testid": testId,
}: SideNavProps) {
  const config = collapsible
  const isCollapsible = config !== undefined
  const controlledCollapsed = config?.isCollapsed
  const [localCollapsed, setLocalCollapsed] = useState(config?.defaultIsCollapsed ?? false)
  const isCollapsed = controlledCollapsed ?? localCollapsed
  const navRef = useRef<HTMLElement | null>(null)
  const collapseLabel = config === undefined || config.hasButton === false
    ? undefined
    : isCollapsed
      ? config.expandLabel
      : config.collapseLabel
  const hasCollapseButton = isCollapsible && config?.hasButton !== false && collapseLabel !== undefined
  const hasFooter = footer !== undefined || footerIcons !== undefined || hasCollapseButton

  const setCollapsed = (next: boolean) => {
    if (next && navRef.current !== null && typeof document !== "undefined") {
      const active = document.activeElement
      if (active instanceof HTMLElement && navRef.current.contains(active)) {
        const activeItem = active.closest<HTMLElement>("li")
        const activePrimary = active.closest<HTMLElement>("[data-side-nav-primary]")
        const ownerItem = activePrimary?.closest<HTMLElement>("li")?.parentElement?.closest<HTMLElement>("li") ?? activeItem
        const owner = ownerItem?.querySelector<HTMLElement>("[data-side-nav-primary]")
        owner?.focus()
      }
    }
    if (controlledCollapsed === undefined) setLocalCollapsed(next)
    config?.onCollapsedChange?.(next)
  }

  return (
    <SideNavContext.Provider value={{ isCollapsed }}>
      <nav
        className={joinClassNames(navClasses.root, isCollapsed && navClasses.rootCollapsed, className)}
        ref={(node) => {
          navRef.current = node
          assignRef(ref, node)
        }}
        id={id}
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
              <button
                className={joinClassNames(navClasses.collapseButton, isCollapsed && navClasses.collapseButtonCollapsed)}
                type="button"
                aria-label={collapseLabel}
                aria-expanded={!isCollapsed}
                onClick={() => setCollapsed(!isCollapsed)}
              >
                {isCollapsed ? <><i aria-hidden="true">‹</i><strong className="sr-only">{collapseLabel}</strong></> : collapseLabel}
              </button>
            ) : null}
            {footerIcons !== undefined ? <menu className={navClasses.footerIcons} role="presentation">{footerIcons}</menu> : null}
          </footer>
        ) : null}
      </nav>
    </SideNavContext.Provider>
  )
}

export default SideNav
