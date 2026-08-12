import type { MouseEvent, ReactNode, Ref } from "react"

/** The supported visual density values mirror Astryx's navigation vocabulary. */
export type NavigationDensity = "compact" | "balanced" | "spacious"

/**
 * Tree levels are intentionally bounded. A bounded union keeps indentation a
 * finite, reviewable contract and prevents dynamic utility-class generation.
 */
export type TreeLevel = 1 | 2 | 3 | 4 | 5 | 6

export type TreeListItemData = {
  readonly id: string
  readonly label: ReactNode
  readonly description?: string
  readonly startContent?: ReactNode
  readonly icon?: ReactNode
  readonly endContent?: ReactNode
  readonly children?: readonly TreeListItemData[]
  readonly href?: string
  readonly target?: string
  readonly isDisabled?: boolean
  readonly isSelected?: boolean
  readonly isExpanded?: boolean
  readonly testId?: string
  readonly onClick?: (event: MouseEvent<HTMLAnchorElement | HTMLButtonElement>) => void
}

/** Adapter result for callers whose domain item shape differs from the tree contract. */
export type TreeListItemAdapterResult<TItem> = Omit<TreeListItemData, "children"> & {
  readonly children?: readonly TItem[]
}

export type TreeListItemAdapter<TItem> = (item: TItem) => TreeListItemAdapterResult<TItem>

export type TreeListAccessibleLabel =
  | { readonly "aria-label": string; readonly header?: ReactNode }
  | { readonly "aria-label"?: never; readonly header: ReactNode }

type TreeListExpansionProps =
  | { readonly expandedIds: readonly string[]; readonly defaultExpandedIds?: never }
  | { readonly expandedIds?: never; readonly defaultExpandedIds?: readonly string[] }

type TreeListCommonProps<TItem> = {
  readonly items: readonly TItem[]
  readonly density?: NavigationDensity
  readonly expandLabel: (item: TreeListItemData) => string
  readonly collapseLabel: (item: TreeListItemData) => string
  readonly variant?: "lineGuides" | "noGuides"
  readonly className?: string
  readonly "data-testid"?: string
  readonly id?: string
  readonly "aria-labelledby"?: string
  readonly ref?: Ref<HTMLElement>
  readonly onExpandedChange?: (id: string, expanded: boolean) => void
  readonly onAction?: (item: TreeListItemData, event?: MouseEvent<HTMLAnchorElement | HTMLButtonElement>) => void
}

type TreeListAdapterProp<TItem> = [TItem] extends [TreeListItemData]
  ? { readonly adapter?: TreeListItemAdapter<TItem> }
  : { readonly adapter: TreeListItemAdapter<TItem> }

export type TreeListProps<TItem = TreeListItemData> = TreeListCommonProps<TItem> &
  TreeListAccessibleLabel &
  TreeListExpansionProps &
  TreeListAdapterProp<TItem>

type SideNavCollapseConfigBase = {
  readonly defaultIsCollapsed?: boolean
  readonly isCollapsed?: boolean
  readonly onCollapsedChange?: (isCollapsed: boolean) => void
}

export type SideNavCollapseConfig =
  | (SideNavCollapseConfigBase & { readonly hasButton?: false; readonly expandLabel?: never; readonly collapseLabel?: never })
  | (SideNavCollapseConfigBase & { readonly hasButton?: true; readonly expandLabel: string; readonly collapseLabel: string })

export type SideNavItemCollapseConfig = {
  readonly defaultIsCollapsed?: boolean
  readonly isCollapsed?: boolean
  readonly onCollapsedChange?: (isCollapsed: boolean) => void
}

export type SideNavProps = {
  readonly children?: ReactNode
  readonly header?: ReactNode
  readonly topContent?: ReactNode
  readonly footer?: ReactNode
  readonly footerIcons?: ReactNode
  readonly collapsible?: SideNavCollapseConfig
  readonly className?: string
  readonly id?: string
  readonly "aria-label"?: string
  readonly "aria-labelledby"?: string
  readonly "data-testid"?: string
  readonly ref?: Ref<HTMLElement>
}

export type SideNavHeadingProps = {
  readonly heading: string
  readonly icon?: ReactNode
  readonly headingHref?: string
  readonly superheading?: string
  readonly superheadingHref?: string
  readonly subheading?: string
  readonly subheadingHref?: string
  readonly headerEndContent?: ReactNode
  readonly className?: string
  readonly "data-testid"?: string
}

type SideNavSectionLabel =
  | { readonly title?: string; readonly heading: string }
  | { readonly title: string; readonly heading?: string }

export type SideNavSectionProps = SideNavSectionLabel & {
  readonly subtitle?: string
  readonly children?: ReactNode
  readonly endContent?: ReactNode
  readonly isHeaderHidden?: boolean
  readonly className?: string
  readonly "data-testid"?: string
}

type SideNavItemCommonProps = {
  readonly label: string
  readonly icon?: ReactNode
  readonly startContent?: ReactNode
  readonly endContent?: ReactNode
  readonly selectedIcon?: ReactNode
  readonly isSelected?: boolean
  readonly isDisabled?: boolean
  readonly href?: string
  readonly target?: string
  readonly onClick?: (event: MouseEvent<HTMLAnchorElement | HTMLButtonElement>) => void
  readonly collapsible?: boolean | SideNavItemCollapseConfig
  readonly defaultIsExpanded?: boolean
  readonly isExpanded?: boolean
  readonly onExpandedChange?: (isExpanded: boolean) => void
  readonly className?: string
  readonly id?: string
  readonly "aria-controls"?: string
  readonly ref?: Ref<HTMLElement>
  readonly size?: "sm" | "md" | "lg"
  readonly "data-testid"?: string
}

export type SideNavItemProps =
  | (SideNavItemCommonProps & {
      readonly children?: never
      readonly expandLabel?: never
      readonly collapseLabel?: never
    })
  | (SideNavItemCommonProps & {
      readonly children: ReactNode
      readonly expandLabel: string
      readonly collapseLabel: string
    })
