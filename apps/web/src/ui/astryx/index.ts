/**
 * Canonical CSP-safe Astryx surface for product consumers.
 *
 * Keep this barrel deliberately explicit. The component-directory barrels also
 * contain implementation helpers, compatibility aliases, and finite class
 * maps; none of those are part of the product-facing API.
 */

export {CheckboxInput} from "./fields/CheckboxInput"
export type {
  CheckboxInputProps,
  CheckboxInputSize,
  CheckboxInputStatus,
  CheckboxInputStatusVariant,
} from "./fields/CheckboxInput"
export {FileInput} from "./fields/FileInput"
export type {
  FileInputProps,
  FileInputStatus,
  FileInputStatusVariant,
  FileInputValidationError,
  FileInputValidationReason,
} from "./fields/FileInput"
export {TextArea} from "./fields/TextArea"
export type {
  TextAreaProps,
  TextAreaStatus,
  TextAreaStatusVariant,
} from "./fields/TextArea"
export {TextInput} from "./fields/TextInput"
export type {
  TextInputProps,
  TextInputStatus,
  TextInputStatusVariant,
  TextInputType,
} from "./fields/TextInput"

export {DateTimeInput} from "./datetime/DateTimeInput"
export type {
  DateTimeChangeResolution,
  DateTimeInputAriaDataProps,
  DateTimeInputProps,
  DateTimeInputSize,
  DateTimeInputStatus,
  DateTimeInputStatusType,
  DateTimeInputTimeIncrement,
  ISODateTimeParts,
  ISODateTimeString,
} from "./datetime/DateTimeInput"

export {Dialog} from "./overlays/Dialog"
export type {
  DialogPlacement,
  DialogProps,
  DialogSize,
} from "./overlays/Dialog"
export {Popover} from "./overlays/Popover"
export type {
  PopoverAlignment,
  PopoverPlacement,
  PopoverProps,
  PopoverSize,
  PopoverTriggerRenderProps,
} from "./overlays/Popover"
export {Tooltip} from "./overlays/Tooltip"
export type {
  TooltipFocusTrigger,
  TooltipProps,
} from "./overlays/Tooltip"
export {DropdownMenu} from "./overlays/DropdownMenu"
export type {
  DropdownMenuButtonProps,
  DropdownMenuDivider,
  DropdownMenuItemData,
  DropdownMenuOption,
  DropdownMenuProps,
  DropdownMenuSection,
} from "./overlays/DropdownMenu"
export {MoreMenu} from "./overlays/MoreMenu"
export type {MoreMenuProps} from "./overlays/MoreMenu"

export {Grid} from "./primitives/Grid"
export type {
  GridAlign,
  GridColumns,
  GridDensity,
  GridGap,
  GridProps,
} from "./primitives/Grid"
export {CodeBlock} from "./primitives/CodeBlock"
export type {
  CodeBlockContainer,
  CodeBlockCopyState,
  CodeBlockHeight,
  CodeBlockProps,
} from "./primitives/CodeBlock"
export {Skeleton} from "./primitives/Skeleton"
export type {SkeletonProps, SkeletonSize} from "./primitives/Skeleton"
export {
  SafeCard,
  SafeHStack,
  SafeLayout,
  SafeLayoutContent,
  SafeLayoutFooter,
  SafeLayoutHeader,
  SafeLayoutPanel,
  SafeMetadataList,
  SafeMetadataListItem,
  SafeSection,
  SafeStack,
  SafeVStack,
} from "./primitives/safe-core"
export type {
  SafeCardProps,
  SafeHStackProps,
  SafeLayoutContentProps,
  SafeLayoutFooterProps,
  SafeLayoutHeaderProps,
  SafeLayoutPanelProps,
  SafeLayoutProps,
  SafeMetadataListColumns,
  SafeMetadataListItemProps,
  SafeMetadataListLabel,
  SafeMetadataListProps,
  SafeSectionProps,
  SafeStackProps,
  SafeVStackProps,
} from "./primitives/safe-core"

export {PageFrame} from "./page-frame/PageFrame"
export type {
  ContentPageFrameProps,
  PageFrameBodyOverflow,
  PageFrameMode,
  PageFrameProps,
  WorkspacePageFrameProps,
} from "./page-frame/PageFrame"

export {Selector} from "./selectors/Selector"
export type {
  SelectorOption,
  SelectorOptionData,
  SelectorOptionType,
  SelectorProps,
  SelectorSection,
} from "./selectors/Selector"
export {MultiSelector} from "./selectors/MultiSelector"
export type {
  MultiSelectorOptionData,
  MultiSelectorOptionType,
  MultiSelectorProps,
  MultiSelectorStatus,
} from "./selectors/MultiSelector"
export {Typeahead} from "./selectors/Typeahead"
export type {TypeaheadProps} from "./selectors/Typeahead"
export {createStaticSource} from "./selectors/search-source"
export type {
  CreateStaticSourceOptions,
  SearchableItem,
  SearchSource,
} from "./selectors/search-source"
export type {
  SelectableOption,
  SelectorDivider,
  SelectorStatus,
} from "./selectors/shared"

export {SideNav} from "./navigation/SideNav"
export {SideNavHeading} from "./navigation/SideNavHeading"
export {SideNavItem} from "./navigation/SideNavItem"
export {SideNavSection} from "./navigation/SideNavSection"
export {TreeList} from "./navigation/TreeList"
export type {
  NavigationDensity,
  SideNavCollapseConfig,
  SideNavHeadingProps,
  SideNavItemCollapseConfig,
  SideNavItemProps,
  SideNavProps,
  SideNavSectionProps,
  TreeLevel,
  TreeListAccessibleLabel,
  TreeListItemAdapter,
  TreeListItemAdapterResult,
  TreeListItemData,
  TreeListProps,
} from "./navigation/types"
