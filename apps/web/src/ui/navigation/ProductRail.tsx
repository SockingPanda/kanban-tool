import { NavigationIcon } from "./icons"
import styles from "./navigation.module.css"
import { mergeNavigationLabels, type NavigationLabels } from "./types"

export type ProductRailItem = "projects" | "settings"

export type ProductRailProps = {
  readonly activeItem?: ProductRailItem
  readonly onNavigate?: (item: ProductRailItem) => void
  readonly brandLabel?: string
  readonly labels?: Partial<NavigationLabels>
  readonly className?: string
}

/** Fixed product-level rail; project diagnostics never become rail peers. */
export function ProductRail({
  activeItem = "projects",
  onNavigate,
  brandLabel = "kanban-tool",
  labels: labelOverrides,
  className,
}: ProductRailProps) {
  const labels = mergeNavigationLabels(labelOverrides)
  const rootClassName = className === undefined ? styles.productRail : `${styles.productRail} ${className}`
  const items: readonly { id: ProductRailItem; label: string; icon: "folder" | "settings" }[] = [
    { id: "projects", label: labels.projects, icon: "folder" },
    { id: "settings", label: labels.settings, icon: "settings" },
  ]

  const renderItem = (item: (typeof items)[number]) => {
    const isActive = item.id === activeItem
    return (
      <button
        key={item.id}
        type="button"
        className={`${styles.railItem} ${isActive ? styles.railItemActive : ""}`}
        aria-current={isActive ? "page" : undefined}
        aria-label={item.label}
        title={item.label}
        onClick={() => onNavigate?.(item.id)}
        data-testid={`product-rail-${item.id}`}
      >
        <NavigationIcon name={item.icon} size={18} />
        <span className={styles.railItemLabel}>{item.label}</span>
      </button>
    )
  }

  return (
    <aside className={rootClassName} aria-label={labels.productNavigation} data-testid="product-rail">
      <span className={styles.brandMark} role="img" aria-label={brandLabel} title={brandLabel}>
        <svg aria-hidden="true" focusable="false" viewBox="0 0 24 24" width="20" height="20">
          <rect x="4" y="4" width="5" height="16" rx="1" fill="currentColor" />
          <rect x="11" y="7" width="4" height="13" rx="1" fill="currentColor" opacity="0.78" />
          <rect x="17" y="10" width="3" height="10" rx="1" fill="currentColor" opacity="0.56" />
        </svg>
      </span>
      <nav className={styles.railNavigation} aria-label={labels.productNavigation}>
        {renderItem(items[0])}
      </nav>
      <nav className={`${styles.railNavigation} ${styles.railNavigationBottom}`} aria-label={`${labels.productNavigation} secondary`}>
        {renderItem(items[1])}
      </nav>
    </aside>
  )
}

export default ProductRail
