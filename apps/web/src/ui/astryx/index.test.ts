import {readFileSync} from "node:fs"

import {describe, expect, test} from "vitest"

import * as publicApi from "./index"
import type {
  CheckboxInputProps,
  CodeBlockProps,
  ContentPageFrameProps,
  CreateStaticSourceOptions,
  DateTimeInputProps,
  DialogProps,
  DropdownMenuProps,
  FileInputProps,
  GridProps,
  MoreMenuProps,
  MultiSelectorProps,
  PageFrameProps,
  PopoverProps,
  SafeCardProps,
  SafeLayoutProps,
  SafeMetadataListProps,
  SafeSectionProps,
  SafeStackProps,
  SafeVStackProps,
  SearchableItem,
  SearchSource,
  SelectorProps,
  SideNavHeadingProps,
  SideNavItemProps,
  SideNavProps,
  SideNavSectionProps,
  SkeletonProps,
  TextAreaProps,
  TextInputProps,
  TooltipProps,
  TreeListItemData,
  TreeListProps,
  TypeaheadProps,
  WorkspacePageFrameProps,
} from "./index"

type PublicTypeWitness = [
  CheckboxInputProps,
  FileInputProps,
  TextAreaProps,
  TextInputProps,
  DateTimeInputProps,
  DialogProps,
  PopoverProps,
  TooltipProps,
  DropdownMenuProps,
  MoreMenuProps,
  GridProps,
  CodeBlockProps,
  SkeletonProps,
  SafeCardProps,
  SafeLayoutProps,
  SafeMetadataListProps,
  SafeSectionProps,
  SafeStackProps,
  SafeVStackProps,
  PageFrameProps,
  ContentPageFrameProps,
  WorkspacePageFrameProps,
  SelectorProps,
  MultiSelectorProps,
  TypeaheadProps<SearchableItem>,
  SearchSource,
  CreateStaticSourceOptions,
  SideNavProps,
  SideNavHeadingProps,
  SideNavItemProps,
  SideNavSectionProps,
  TreeListProps,
  TreeListItemData,
]

const publicTypeWitness: PublicTypeWitness | undefined = undefined
void publicTypeWitness

describe("canonical CSP-safe Astryx root barrel", () => {
  test("exposes only the curated runtime namespace", () => {
    expect(Object.keys(publicApi).sort()).toEqual([
      "CheckboxInput",
      "CodeBlock",
      "DateTimeInput",
      "Dialog",
      "DropdownMenu",
      "FileInput",
      "Grid",
      "MoreMenu",
      "MultiSelector",
      "PageFrame",
      "Popover",
      "SafeCard",
      "SafeHStack",
      "SafeLayout",
      "SafeLayoutContent",
      "SafeLayoutFooter",
      "SafeLayoutHeader",
      "SafeLayoutPanel",
      "SafeMetadataList",
      "SafeMetadataListItem",
      "SafeSection",
      "SafeStack",
      "SafeVStack",
      "Selector",
      "SideNav",
      "SideNavHeading",
      "SideNavItem",
      "SideNavSection",
      "Skeleton",
      "TextArea",
      "TextInput",
      "Tooltip",
      "TreeList",
      "Typeahead",
      "createStaticSource",
    ])
  })

  test("keeps aliases, guards, maps, and runtime helpers out of the source contract", () => {
    const source = readFileSync(new URL("./index.ts", import.meta.url), "utf8")

    expect(source).not.toMatch(/\bAstryx(?:Dialog|Popover|Tooltip|DropdownMenu|MoreMenu)\b/)
    expect(source).not.toMatch(/\bStatic(?:CoreProps|Card|HStack|Layout|LayoutContent|LayoutFooter|LayoutHeader|LayoutPanel|MetadataList|MetadataListItem|Section|Stack|VStack)\b/)
    expect(source).not.toMatch(/\b(?:guardNoRuntimeStyleProps|stripNoRuntimeStyleProps|pickCspSafeDomProps|CspSafeDomProps)\b/)
    expect(source).not.toMatch(/\b(?:DIALOG|POPOVER|DROPDOWN_MENU|GRID|CODE_BLOCK|SKELETON|PAGE_FRAME)_[A-Z0-9_]+\b/)
    expect(source).not.toMatch(/\b(?:classNames|getFocusableElements|supportsNativePopover|scheduleCopyFeedbackReset|mergeDescribedBy)\b/)
    expect(source).not.toMatch(/\.\/(?:context|icons|constants|interaction|overlay-runtime|shared|dom-props)/)
  })
})
