import { cloneElement, isValidElement, useState, type MouseEvent, type ReactElement, type ReactNode } from "react"

import { joinClassNames } from "./classNames"
import { useSideNavContext } from "./context"
import type { SideNavItemProps } from "./types"

const itemClasses = {
  item: "list-none",
  row: "flex min-w-0 items-center gap-2 rounded-md px-3 py-2 text-sm text-primary outline-none hover:bg-overlay-hover focus-visible:outline-2 focus-visible:outline-accent",
  rowSm: "py-1.5",
  rowLg: "py-3",
  selected: "bg-accent-muted font-semibold text-accent",
  disabled: "cursor-not-allowed opacity-50",
  collapsedRow: "justify-center px-2",
  label: "min-w-0 flex-1 truncate text-left",
  collapsedLabel: "sr-only",
  endContent: "ml-auto shrink-0",
  nestedList: "m-0 flex list-none flex-col gap-1 p-0",
  toggle: "shrink-0 rounded-md border-0 bg-transparent p-0.5 text-secondary outline-none focus-visible:outline-2 focus-visible:outline-accent",
} as const

function iconNode(icon: ReactNode): ReactNode {
  return isValidElement(icon) ? cloneElement(icon as ReactElement<{ "aria-hidden"?: boolean }>, { "aria-hidden": true }) : icon
}

export function SideNavItem({
  label,
  icon,
  startContent,
  endContent,
  selectedIcon,
  isSelected = false,
  isDisabled = false,
  href,
  target,
  onClick,
  children,
  collapsible = false,
  defaultIsExpanded = true,
  isExpanded: controlledExpanded,
  onExpandedChange,
  expandLabel,
  collapseLabel,
  className,
  size = "md",
  "data-testid": testId,
}: SideNavItemProps) {
  const { isCollapsed } = useSideNavContext()
  const collapseConfig = typeof collapsible === "object" ? collapsible : {}
  const configControlledExpanded = collapseConfig.isCollapsed === undefined ? undefined : !collapseConfig.isCollapsed
  const initialExpanded = collapseConfig.defaultIsCollapsed === undefined ? defaultIsExpanded : !collapseConfig.defaultIsCollapsed
  const controlledExpandedValue = controlledExpanded ?? configControlledExpanded
  const [localExpanded, setLocalExpanded] = useState(initialExpanded)
  const hasChildren = children !== undefined && children !== null
  const expanded = controlledExpandedValue ?? localExpanded
  const canToggle = hasChildren && collapsible !== false
  const hasPrimaryAction = href !== undefined || onClick !== undefined
  const visibleIcon = isSelected && selectedIcon !== undefined ? selectedIcon : startContent ?? icon
  const rowClassName = joinClassNames(
    itemClasses.row,
    size === "sm" && itemClasses.rowSm,
    size === "lg" && itemClasses.rowLg,
    isSelected && itemClasses.selected,
    isDisabled && itemClasses.disabled,
    isCollapsed && itemClasses.collapsedRow,
    className,
  )
  const labelClassName = joinClassNames(itemClasses.label, isCollapsed && itemClasses.collapsedLabel)

  const toggleExpanded = () => {
    const next = !expanded
    if (controlledExpandedValue === undefined) setLocalExpanded(next)
    onExpandedChange?.(next)
    collapseConfig.onCollapsedChange?.(!next)
  }

  const handleClick = (event: MouseEvent<HTMLAnchorElement | HTMLButtonElement>) => {
    if (isDisabled) {
      event.preventDefault()
      return
    }
    if (canToggle && !hasPrimaryAction && !isCollapsed) {
      event.preventDefault()
      toggleExpanded()
      return
    }
    onClick?.(event)
  }

  const content = (
    <>
      {visibleIcon !== undefined ? iconNode(visibleIcon) : null}
      <strong className={labelClassName}>{label}</strong>
      {!isCollapsed && endContent !== undefined ? <aside className={itemClasses.endContent}>{endContent}</aside> : null}
    </>
  )

  const action = href !== undefined ? (
    <a
      className={rowClassName}
      href={isDisabled ? undefined : href}
      target={isDisabled ? undefined : target}
      aria-current={isSelected ? "page" : undefined}
      aria-disabled={isDisabled ? "true" : undefined}
      aria-label={isCollapsed ? label : undefined}
      data-testid={testId}
      onClick={handleClick}
    >
      {content}
    </a>
  ) : (
    <button
      className={rowClassName}
      type="button"
      disabled={isDisabled}
      aria-current={isSelected ? "page" : undefined}
      aria-disabled={isDisabled ? "true" : undefined}
      aria-label={isCollapsed ? label : undefined}
      data-testid={testId}
      onClick={handleClick}
    >
      {content}
    </button>
  )

  return (
    <li className={itemClasses.item} data-collapsed={isCollapsed ? "true" : "false"}>
      {canToggle ? (
        <menu className="m-0 flex list-none items-center gap-1 p-0">
          {action}
          <button className={itemClasses.toggle} type="button" aria-label={expanded ? collapseLabel : expandLabel} aria-expanded={expanded} onClick={toggleExpanded} tabIndex={-1}>
            {expanded ? "−" : "+"}
          </button>
        </menu>
      ) : action}
      {!isCollapsed && hasChildren && expanded ? <ul className={itemClasses.nestedList}>{children}</ul> : null}
    </li>
  )
}

export default SideNavItem
