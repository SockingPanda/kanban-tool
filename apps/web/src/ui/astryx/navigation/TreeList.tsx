import { useCallback, useEffect, useId, useMemo, useRef, useState, type FocusEvent, type MouseEvent, type ReactNode } from "react"

import { useTreeFocus } from "@astryxdesign/core/hooks"

import { joinClassNames } from "./classNames"
import { TREE_GUIDE_CLASSES, TREE_LEVEL_CLASSES } from "./constants"
import type {
  NavigationDensity,
  TreeLevel,
  TreeListItemAdapter,
  TreeListItemAdapterResult,
  TreeListItemData,
  TreeListProps,
} from "./types"

/**
 * Finite level-to-utility mapping. Keeping every utility literal makes the
 * generated CSS discoverable by the existing Tailwind bridge.
 */
const densityClasses: Readonly<Record<NavigationDensity, string>> = {
  compact: "py-1",
  balanced: "py-2",
  spacious: "py-3",
}

const treeClasses = {
  root: "min-w-0",
  header: "mb-2",
  tree: "m-0 flex list-none flex-col gap-1 p-0",
  item: "list-none outline-none focus-visible:outline-2 focus-visible:outline-accent",
  row: "flex min-w-0 items-center gap-2 rounded-md pr-2 text-sm text-primary outline-none hover:bg-overlay-hover focus-visible:outline-2 focus-visible:outline-accent",
  selected: "bg-accent-muted font-semibold text-accent",
  disabled: "cursor-not-allowed opacity-50",
  label: "min-w-0 flex-1 truncate text-left",
  description: "min-w-0 flex-1 truncate text-xs text-secondary",
  content: "min-w-0 flex-1",
  start: "shrink-0",
  end: "ml-auto shrink-0",
  link: "flex min-w-0 flex-1 items-center gap-2 no-underline outline-none focus-visible:outline-2 focus-visible:outline-accent",
  action: "flex min-w-0 flex-1 items-center gap-2 rounded-md border-0 bg-transparent p-0 text-left outline-none focus-visible:outline-2 focus-visible:outline-accent",
  toggle: "shrink-0 rounded-md border-0 bg-transparent p-1 text-secondary outline-none focus-visible:outline-2 focus-visible:outline-accent",
  group: "m-0 flex list-none flex-col gap-1 p-0",
} as const

const LEVELS: readonly TreeLevel[] = [1, 2, 3, 4, 5, 6]

function nextLevel(level: TreeLevel): TreeLevel {
  const index = LEVELS.indexOf(level)
  return LEVELS[Math.min(index + 1, LEVELS.length - 1)] ?? 6
}

function collectExpanded(items: readonly TreeListItemData[], output = new Set<string>()): Set<string> {
  for (const item of items) {
    if (item.isExpanded === true && item.children !== undefined && item.children.length > 0) output.add(item.id)
    if (item.children !== undefined) collectExpanded(item.children, output)
  }
  return output
}

function normalizeItems<TItem>(items: readonly TItem[], adapter?: TreeListItemAdapter<TItem>): TreeListItemData[] {
  const adapt = adapter ?? ((item: TItem) => item as unknown as TreeListItemAdapterResult<TItem>)
  return items.map((item) => {
    const adapted = adapt(item)
    const { children, ...itemData } = adapted
    return children === undefined
      ? itemData
      : { ...itemData, children: normalizeItems(children, adapter) }
  })
}

function findSeed(items: readonly TreeListItemData[]): string | undefined {
  let firstEnabled: string | undefined
  const walk = (entries: readonly TreeListItemData[]): string | undefined => {
    for (const item of entries) {
      if (firstEnabled === undefined && item.isDisabled !== true) firstEnabled = item.id
      if (item.isSelected === true && item.isDisabled !== true) return item.id
      if (item.isExpanded === true && item.children !== undefined) {
        const selected = walk(item.children)
        if (selected !== undefined) return selected
      }
    }
    return undefined
  }
  return walk(items) ?? firstEnabled
}

function itemText(item: TreeListItemData): ReactNode {
  return item.description === undefined ? item.label : (
    <section className={treeClasses.content}>
      <strong className={treeClasses.label}>{item.label}</strong>
      <small className={treeClasses.description}>{item.description}</small>
    </section>
  )
}

type RenderItemProps = {
  readonly item: TreeListItemData
  readonly level: TreeLevel
  readonly position: number
  readonly setSize: number
  readonly density: NavigationDensity
  readonly expandLabel: (item: TreeListItemData) => string
  readonly collapseLabel: (item: TreeListItemData) => string
  readonly expanded: boolean
  readonly isTabbable: boolean
  readonly onToggle: (item: TreeListItemData, expanded: boolean) => void
  readonly onAction?: (item: TreeListItemData) => void
  readonly renderChildren: (items: readonly TreeListItemData[], level: TreeLevel) => ReactNode
}

function TreeItem({
  item,
  level,
  position,
  setSize,
  density,
  expandLabel,
  collapseLabel,
  expanded,
  isTabbable,
  onToggle,
  onAction,
  renderChildren,
}: RenderItemProps) {
  const hasChildren = item.children !== undefined && item.children.length > 0
  const isDisabled = item.isDisabled === true
  const handleAction = (event: MouseEvent<HTMLAnchorElement | HTMLButtonElement>) => {
    if (isDisabled) {
      event.preventDefault()
      return
    }
    item.onClick?.(event)
    onAction?.(item)
  }
  const rowClassName = joinClassNames(
    treeClasses.row,
    densityClasses[density],
    TREE_LEVEL_CLASSES[level],
    item.isSelected === true && treeClasses.selected,
    isDisabled && treeClasses.disabled,
  )
  const content = (
    <>
      {item.startContent !== undefined || item.icon !== undefined ? <i className={treeClasses.start} aria-hidden="true">{item.startContent ?? item.icon}</i> : null}
      <section className={treeClasses.content}>{itemText(item)}</section>
      {item.endContent !== undefined ? <aside className={treeClasses.end}>{item.endContent}</aside> : null}
    </>
  )
  const action = item.href !== undefined ? (
    <a
      className={joinClassNames(rowClassName, treeClasses.link)}
      href={isDisabled ? undefined : item.href}
      target={isDisabled ? undefined : item.target}
      aria-current={item.isSelected === true ? "page" : undefined}
      aria-disabled={isDisabled ? "true" : undefined}
      data-testid={item.testId}
      onClick={handleAction}
      tabIndex={-1}
    >
      {content}
    </a>
  ) : item.onClick !== undefined || onAction !== undefined ? (
    <button className={joinClassNames(rowClassName, treeClasses.action)} type="button" disabled={isDisabled} data-testid={item.testId} data-tree-action="true" onClick={handleAction} tabIndex={-1}>
      {content}
    </button>
  ) : (
    <section className={rowClassName} data-testid={item.testId}>{content}</section>
  )

  return (
    <li
      className={treeClasses.item}
      role="treeitem"
      aria-level={level}
      aria-posinset={position}
      aria-setsize={setSize}
      aria-expanded={hasChildren ? expanded : undefined}
      aria-selected={item.isSelected === true ? true : undefined}
      aria-disabled={isDisabled ? true : undefined}
      data-tree-id={item.id}
      data-tree-level={level}
      data-tree-disabled={isDisabled ? "true" : undefined}
      tabIndex={isDisabled || !isTabbable ? -1 : 0}
    >
      <menu className="m-0 flex list-none items-center gap-1 p-0">
        {hasChildren ? (
          <button
            className={treeClasses.toggle}
            type="button"
            aria-label={expanded ? collapseLabel(item) : expandLabel(item)}
            aria-expanded={expanded}
            data-tree-toggle="true"
            disabled={isDisabled}
            tabIndex={-1}
            onClick={() => onToggle(item, !expanded)}
          >
            {expanded ? "−" : "+"}
          </button>
        ) : null}
        {action}
      </menu>
      {hasChildren && expanded ? renderChildren(item.children ?? [], nextLevel(level)) : null}
    </li>
  )
}

export function TreeList<TItem = TreeListItemData>({
  items,
  adapter,
  density = "balanced",
  header,
  expandLabel,
  collapseLabel,
  variant = "lineGuides",
  className,
  "data-testid": testId,
  "aria-label": ariaLabel,
  onExpandedChange,
  onAction,
}: TreeListProps<TItem>) {
  const normalizedItems = useMemo(() => normalizeItems(items, adapter), [adapter, items])
  const headingId = useId()
  const expandedFromProps = useMemo(() => collectExpanded(normalizedItems), [normalizedItems])
  const [expandedOverrides, setExpandedOverrides] = useState<ReadonlyMap<string, boolean>>(() => new Map())
  const previousExpandedFromProps = useRef(expandedFromProps)
  const seedId = useMemo(() => findSeed(normalizedItems), [normalizedItems])
  const [activeId, setActiveId] = useState<string | undefined>(seedId)
  const itemById = useMemo(() => {
    const map = new Map<string, TreeListItemData>()
    const walk = (entries: readonly TreeListItemData[]) => {
      for (const item of entries) {
        map.set(item.id, item)
        if (item.children !== undefined) walk(item.children)
      }
    }
    walk(normalizedItems)
    return map
  }, [normalizedItems])

  const isExpanded = useCallback((item: TreeListItemData) => expandedOverrides.has(item.id) ? expandedOverrides.get(item.id) === true : expandedFromProps.has(item.id), [expandedFromProps, expandedOverrides])
  const visibleIds = useMemo(() => {
    const ids = new Set<string>()
    const walk = (entries: readonly TreeListItemData[]) => {
      for (const item of entries) {
        ids.add(item.id)
        if (item.children !== undefined && (expandedOverrides.has(item.id) ? expandedOverrides.get(item.id) === true : expandedFromProps.has(item.id))) {
          walk(item.children)
        }
      }
    }
    walk(normalizedItems)
    return ids
  }, [expandedFromProps, expandedOverrides, normalizedItems])
  const visibleSeedId = useMemo(() => {
    let first: string | undefined
    const walk = (entries: readonly TreeListItemData[]) => {
      for (const item of entries) {
        if (first === undefined && item.isDisabled !== true) first = item.id
        if (first !== undefined) return
        if (item.children !== undefined && (expandedOverrides.has(item.id) ? expandedOverrides.get(item.id) === true : expandedFromProps.has(item.id))) walk(item.children)
      }
    }
    walk(normalizedItems)
    return first
  }, [expandedFromProps, expandedOverrides, normalizedItems])
  useEffect(() => {
    setActiveId((current) => current !== undefined && visibleIds.has(current) ? current : visibleSeedId)
  }, [visibleIds, visibleSeedId])
  useEffect(() => {
    const previous = previousExpandedFromProps.current
    if (previous === expandedFromProps) return
    setExpandedOverrides((current) => {
      const next = new Map(current)
      const ids = new Set([...previous, ...expandedFromProps])
      for (const id of ids) {
        if (previous.has(id) !== expandedFromProps.has(id)) next.delete(id)
      }
      return next.size === current.size ? current : next
    })
    previousExpandedFromProps.current = expandedFromProps
  }, [expandedFromProps])

  const handleToggle = useCallback((item: TreeListItemData, next: boolean) => {
    setExpandedOverrides((current) => {
      const nextOverrides = new Map(current)
      nextOverrides.set(item.id, next)
      return nextOverrides
    })
    onExpandedChange?.(item.id, next)
  }, [onExpandedChange])
  const activateItem = useCallback((treeItem: HTMLElement, id: string | undefined): boolean => {
    const item = id === undefined ? undefined : itemById.get(id)
    if (item === undefined || item.isDisabled === true) return false
    const action = Array.from(treeItem.querySelectorAll<HTMLElement>("a[href], button[data-tree-action]"))
      .find((candidate) => candidate.closest('[role="treeitem"]') === treeItem)
    if (action !== undefined) {
      action.click()
      return true
    }
    if (item.href !== undefined || item.onClick !== undefined || onAction !== undefined) {
      onAction?.(item)
      return true
    }
    return false
  }, [itemById, onAction])
  const { treeRef, handleKeyDown, handleFocus } = useTreeFocus<HTMLUListElement>({
    onToggleExpand: (id) => {
      const item = itemById.get(id)
      if (item !== undefined) handleToggle(item, !isExpanded(item))
    },
    onActivate: activateItem,
    onActiveChange: (id) => {
      if (id !== undefined) setActiveId(id)
    },
    hasRovingTabIndex: true,
  })
  const handleTreeFocus = useCallback((event: FocusEvent<HTMLUListElement>) => {
    const target = event.target instanceof HTMLElement ? event.target.closest('[role="treeitem"]') : null
    const id = target?.getAttribute("data-tree-id")
    if (id !== null && id !== undefined) setActiveId(id)
    handleFocus(event)
  }, [handleFocus])

  const renderChildren = (entries: readonly TreeListItemData[], level: TreeLevel): ReactNode => (
    <ul className={joinClassNames(treeClasses.group, variant === "lineGuides" && TREE_GUIDE_CLASSES[level])} role="group">
      {entries.map((item, index) => (
        <TreeItem
          key={item.id}
          item={item}
          level={level}
          position={index + 1}
          setSize={entries.length}
          density={density}
          expandLabel={expandLabel}
          collapseLabel={collapseLabel}
          expanded={isExpanded(item)}
          isTabbable={item.id === activeId}
          onToggle={handleToggle}
          onAction={onAction}
          renderChildren={renderChildren}
        />
      ))}
    </ul>
  )

  return (
    <section className={joinClassNames(treeClasses.root, className)} data-testid={testId} data-tree-variant={variant}>
      {header !== undefined ? <header className={treeClasses.header} id={headingId}>{header}</header> : null}
      <ul
        ref={treeRef}
        className={treeClasses.tree}
        role="tree"
        aria-label={ariaLabel}
        aria-labelledby={header === undefined ? undefined : headingId}
        onKeyDown={handleKeyDown}
        onFocus={handleTreeFocus}
      >
        {normalizedItems.map((item, index) => (
          <TreeItem
            key={item.id}
            item={item}
            level={1}
            position={index + 1}
            setSize={normalizedItems.length}
            density={density}
            expandLabel={expandLabel}
            collapseLabel={collapseLabel}
            expanded={isExpanded(item)}
            isTabbable={item.id === activeId}
            onToggle={handleToggle}
            onAction={onAction}
            renderChildren={renderChildren}
          />
        ))}
      </ul>
    </section>
  )
}

export default TreeList
