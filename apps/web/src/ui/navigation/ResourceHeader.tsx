import type { ReactNode } from "react"

import { NavigationIcon } from "./icons"
import styles from "./navigation.module.css"
import { mergeNavigationLabels, type BreadcrumbItem, type NavigationLabels, type ResourceHeaderAction, type ResourceHeaderMoreItem } from "./types"

export type BreadcrumbProps = {
  readonly items: readonly BreadcrumbItem[]
  readonly labels?: Partial<NavigationLabels>
  readonly className?: string
}

export function Breadcrumb({ items, labels: labelOverrides, className }: BreadcrumbProps) {
  const labels = mergeNavigationLabels(labelOverrides)
  const rootClassName = className === undefined ? styles.breadcrumb : `${styles.breadcrumb} ${className}`
  return (
    <nav className={rootClassName} aria-label={labels.breadcrumb} data-testid="resource-breadcrumb">
      <ol className={styles.breadcrumbList}>
        {items.map((item, index) => {
          const isCurrent = index === items.length - 1
          return (
            <li key={`${item.label}-${index}`} className={styles.breadcrumbItem} aria-current={isCurrent ? "page" : undefined}>
              {index > 0 ? <span className={styles.breadcrumbSeparator} aria-hidden="true">/</span> : null}
              {item.href !== undefined && !isCurrent ? <a href={item.href}>{item.label}</a> : <span>{item.label}</span>}
            </li>
          )
        })}
      </ol>
    </nav>
  )
}

export type ResourceHeaderProps = {
  readonly breadcrumbs: readonly BreadcrumbItem[]
  readonly projectSwitchLabel?: string
  readonly onProjectSwitch?: () => void
  readonly projectSwitchAriaLabel?: string
  readonly actions?: readonly ResourceHeaderAction[]
  readonly moreItems?: readonly ResourceHeaderMoreItem[]
  readonly children?: ReactNode
  readonly onMenuToggle?: () => void
  readonly menuOpen?: boolean
  /** Explicit ID seam shared with ProjectsSidebar's drawer element. */
  readonly menuControlsId?: string
  readonly labels?: Partial<NavigationLabels>
  readonly className?: string
}

function actionClassName(action: ResourceHeaderAction): string {
  const tone = action.kind === "primary"
    ? styles.resourceHeaderActionPrimary
    : action.kind === "ghost"
      ? styles.resourceHeaderActionGhost
      : ""
  return `${styles.resourceHeaderAction} ${tone}`
}

function HeaderAction({ action }: { readonly action: ResourceHeaderAction }) {
  const disabled = action.disabled === true || action.onSelect === undefined
  return (
    <button
      type="button"
      className={actionClassName(action)}
      disabled={disabled}
      onClick={disabled ? undefined : action.onSelect}
      data-testid={`resource-header-action-${action.id}`}
    >
      {action.icon}
      <span>{action.label}</span>
    </button>
  )
}

function MoreItem({ item }: { readonly item: ResourceHeaderMoreItem }) {
  const disabled = item.disabled === true || (item.href === undefined && item.onSelect === undefined)
  const content = (
    <>
      {item.icon}
      <span className={styles.resourceHeaderMoreItemLabel}>{item.label}</span>
    </>
  )
  if (item.href !== undefined && !disabled) {
    return (
      <a className={styles.resourceHeaderMoreItem} href={item.href} onClick={item.onSelect}>
        {content}
      </a>
    )
  }
  return (
    <button type="button" className={styles.resourceHeaderMoreItem} disabled={disabled} onClick={disabled ? undefined : item.onSelect}>
      {content}
    </button>
  )
}

/** Compact breadcrumb/header composition shared by collection and project surfaces. */
export function ResourceHeader({
  breadcrumbs,
  projectSwitchLabel,
  onProjectSwitch,
  projectSwitchAriaLabel,
  actions = [],
  moreItems = [],
  children,
  onMenuToggle,
  menuOpen = false,
  menuControlsId = "projects-sidebar",
  labels: labelOverrides,
  className,
}: ResourceHeaderProps) {
  const labels = mergeNavigationLabels(labelOverrides)
  const rootClassName = className === undefined ? styles.resourceHeader : `${styles.resourceHeader} ${className}`

  return (
    <header className={rootClassName} data-testid="resource-header">
      <div className={styles.resourceHeaderMain}>
        {onMenuToggle !== undefined ? (
          <button
            type="button"
            className={`${styles.iconButton} ${styles.resourceHeaderMenu}`}
            aria-label={menuOpen ? labels.closeProjectNavigation : labels.openProjectNavigation}
            aria-controls={menuControlsId}
            aria-expanded={menuOpen}
            aria-haspopup="dialog"
            onClick={onMenuToggle}
            data-testid="resource-header-menu"
          >
            <NavigationIcon name="menu" size={18} />
          </button>
        ) : null}
        <Breadcrumb items={breadcrumbs} labels={labels} />
      </div>

      <div className={styles.resourceHeaderActions}>
        {projectSwitchLabel !== undefined ? (
          <button
            type="button"
            className={styles.resourceHeaderProjectSwitch}
            aria-label={projectSwitchAriaLabel ?? labels.projectPicker}
            onClick={onProjectSwitch}
            disabled={onProjectSwitch === undefined}
            data-testid="resource-header-project-switch"
          >
            <NavigationIcon name="folder" size={16} />
            <span className={styles.resourceHeaderProjectSwitchLabel}>{projectSwitchLabel}</span>
            <NavigationIcon name="chevron-down" size={15} />
          </button>
        ) : null}
        {actions.map((action) => <HeaderAction key={action.id} action={action} />)}
        {children}
        {moreItems.length > 0 ? (
          <details className={styles.resourceHeaderMore}>
            <summary className={styles.resourceHeaderMoreTrigger}>
              <NavigationIcon name="activity" size={16} />
              <span>{labels.more}</span>
              <NavigationIcon name="chevron-down" size={14} />
            </summary>
            <div className={styles.resourceHeaderMoreMenu} aria-label={labels.more}>
              {moreItems.map((item) => <MoreItem key={item.id} item={item} />)}
            </div>
          </details>
        ) : null}
      </div>
    </header>
  )
}

export default ResourceHeader
