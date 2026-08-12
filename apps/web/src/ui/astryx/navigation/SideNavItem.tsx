import { cloneElement, isValidElement, useId, useLayoutEffect, useRef, useState, type FocusEvent, type MouseEvent, type ReactElement, type ReactNode, type Ref } from "react"

import { joinClassNames } from "./classNames"
import { useSideNavContext } from "./context"
import { isPrimaryNavigationClick } from "./interaction"
import type { SideNavItemProps } from "./types"

const itemClasses = {
  item: "list-none",
  row: "flex min-w-0 items-center gap-2 rounded-md px-3 py-2 text-sm text-primary outline-none hover:bg-overlay-hover focus-visible:outline-2 focus-visible:outline-accent",
  rowSm: "py-1.5",
  rowLg: "py-3",
  selected: "bg-accent-muted font-semibold text-accent",
  disabled: "cursor-not-allowed opacity-50",
  collapsedRow: "justify-center px-2",
  label: "min-w-0 flex-1 truncate text-start",
  collapsedLabel: "sr-only",
  startContent: "shrink-0",
  endContent: "ms-auto shrink-0",
  nestedList: "m-0 flex list-none flex-col gap-1 p-0",
  toggle: "shrink-0 rounded-md border-0 bg-transparent p-0.5 text-secondary outline-none focus-visible:outline-2 focus-visible:outline-accent",
} as const

function assignRef<T>(ref: Ref<T> | undefined | null, value: T | null): void {
  if (typeof ref === "function") ref(value)
  else if (ref !== undefined && ref !== null) ref.current = value
}

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
  collapsible = true,
  defaultIsExpanded = true,
  isExpanded: controlledExpanded,
  onExpandedChange,
  expandLabel,
  collapseLabel,
  id,
  "aria-controls": ariaControls,
  ref,
  className,
  size = "md",
  "data-testid": testId,
}: SideNavItemProps) {
  const { isCollapsed } = useSideNavContext()
  const generatedId = useId()
  const primaryRef = useRef<HTMLElement | null>(null)
  const nestedListRef = useRef<HTMLUListElement | null>(null)
  const nestedHadFocus = useRef(false)
  const collapseConfig = typeof collapsible === "object" ? collapsible : {}
  const configControlledExpanded = collapseConfig.isCollapsed === undefined ? undefined : !collapseConfig.isCollapsed
  const initialExpanded = collapseConfig.defaultIsCollapsed === undefined ? defaultIsExpanded : !collapseConfig.defaultIsCollapsed
  const controlledExpandedValue = controlledExpanded ?? configControlledExpanded
  const [localExpanded, setLocalExpanded] = useState(initialExpanded)
  const hasChildren = children !== undefined && children !== null
  const expanded = controlledExpandedValue ?? localExpanded
  const canToggle = hasChildren && collapsible !== false
  const hasPrimaryAction = href !== undefined || onClick !== undefined
  const hasIndependentToggle = canToggle && hasPrimaryAction
  const resolvedAriaControls = hasChildren ? ariaControls ?? `side-nav-item-${generatedId}` : undefined
  const renderedAriaControls = !isCollapsed && hasChildren && expanded ? resolvedAriaControls : undefined
  useLayoutEffect(() => {
    if ((!expanded || isCollapsed) && nestedHadFocus.current) {
      primaryRef.current?.focus()
      nestedHadFocus.current = false
    }
  }, [expanded, isCollapsed])
  const visibleIcon = isSelected && selectedIcon !== undefined ? selectedIcon : icon
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
    if (!next && nestedListRef.current !== null && typeof document !== "undefined") {
      const active = document.activeElement
      if (active instanceof HTMLElement && nestedListRef.current.contains(active)) {
        primaryRef.current?.focus()
      }
    }
    if (controlledExpandedValue === undefined) setLocalExpanded(next)
    onExpandedChange?.(next)
    collapseConfig.onCollapsedChange?.(!next)
  }

  const handleClick = (event: MouseEvent<HTMLAnchorElement | HTMLButtonElement>) => {
    if (event.defaultPrevented) return
    if (isDisabled) {
      event.preventDefault()
      return
    }
    if (href !== undefined && !isPrimaryNavigationClick(event)) return
    if (canToggle && !hasPrimaryAction && !isCollapsed) {
      event.preventDefault()
      toggleExpanded()
      return
    }
    onClick?.(event)
  }

  const content = (
    <>
      {visibleIcon !== undefined ? <i aria-hidden="true">{iconNode(visibleIcon)}</i> : null}
      {startContent !== undefined ? <section className={itemClasses.startContent} role="presentation">{startContent}</section> : null}
      <strong className={labelClassName}>{label}</strong>
      {!isCollapsed && endContent !== undefined ? <section className={itemClasses.endContent} role="presentation">{endContent}</section> : null}
    </>
  )

  const action = href !== undefined ? (
    <a
      className={rowClassName}
      ref={(node) => {
        primaryRef.current = node
        assignRef(ref, node)
      }}
      id={id}
      href={isDisabled ? undefined : href}
      target={isDisabled ? undefined : target}
      aria-current={isSelected ? "page" : undefined}
      aria-disabled={isDisabled ? "true" : undefined}
      aria-label={canToggle && !hasPrimaryAction ? (expanded ? collapseLabel : expandLabel) : isCollapsed ? label : undefined}
      aria-controls={renderedAriaControls}
      aria-expanded={canToggle && !hasPrimaryAction ? expanded : undefined}
      data-side-nav-primary="true"
      data-testid={testId}
      onClick={handleClick}
    >
      {content}
    </a>
  ) : (
    <button
      className={rowClassName}
      ref={(node) => {
        primaryRef.current = node
        assignRef(ref, node)
      }}
      id={id}
      type="button"
      disabled={isDisabled}
      aria-current={isSelected ? "page" : undefined}
      aria-disabled={isDisabled ? "true" : undefined}
      aria-label={canToggle && !hasPrimaryAction ? (expanded ? collapseLabel : expandLabel) : isCollapsed ? label : undefined}
      aria-controls={renderedAriaControls}
      aria-expanded={canToggle && !hasPrimaryAction ? expanded : undefined}
      data-side-nav-primary="true"
      data-testid={testId}
      onClick={handleClick}
    >
      {content}
    </button>
  )

  return (
    <li className={itemClasses.item} data-side-nav-item="true" data-collapsed={isCollapsed ? "true" : "false"}>
      {canToggle ? (
        <menu className="m-0 flex list-none items-center gap-1 p-0" role="presentation">
          {action}
          {hasIndependentToggle ? (
            <button className={itemClasses.toggle} type="button" aria-label={expanded ? collapseLabel : expandLabel} aria-expanded={expanded} aria-controls={renderedAriaControls} onClick={toggleExpanded} tabIndex={0}>
              {expanded ? "−" : "+"}
            </button>
          ) : null}
        </menu>
      ) : action}
      {!isCollapsed && hasChildren && expanded ? (
        <ul
          ref={nestedListRef}
          id={renderedAriaControls}
          className={itemClasses.nestedList}
          onFocus={() => { nestedHadFocus.current = true }}
          onBlur={(event: FocusEvent<HTMLUListElement>) => {
            const next = event.relatedTarget
            if (!(next instanceof Node) || !event.currentTarget.contains(next)) nestedHadFocus.current = false
          }}
        >
          {children}
        </ul>
      ) : null}
    </li>
  )
}

export default SideNavItem
