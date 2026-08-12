import { createElement, type ComponentType } from "react"

import type { CardProps } from "@astryxdesign/core/Card"
import { Card as AstryxCard } from "@astryxdesign/core/Card"
import type { HStackProps } from "@astryxdesign/core/HStack"
import { HStack as AstryxHStack } from "@astryxdesign/core/HStack"
import type {
  LayoutContentProps,
  LayoutFooterProps,
  LayoutHeaderProps,
  LayoutPanelProps,
  LayoutProps,
} from "@astryxdesign/core/Layout"
import {
  Layout as AstryxLayout,
  LayoutContent as AstryxLayoutContent,
  LayoutFooter as AstryxLayoutFooter,
  LayoutHeader as AstryxLayoutHeader,
  LayoutPanel as AstryxLayoutPanel,
} from "@astryxdesign/core/Layout"
import type { MetadataListItemProps, MetadataListProps } from "@astryxdesign/core/MetadataList"
import {
  MetadataList as AstryxMetadataList,
  MetadataListItem as AstryxMetadataListItem,
} from "@astryxdesign/core/MetadataList"
import type { SectionProps } from "@astryxdesign/core/Section"
import { Section as AstryxSection } from "@astryxdesign/core/Section"
import type { StackProps } from "@astryxdesign/core/Stack"
import { Stack as AstryxStack } from "@astryxdesign/core/Stack"
import type { VStackProps } from "@astryxdesign/core/VStack"
import { VStack as AstryxVStack } from "@astryxdesign/core/VStack"

/**
 * Props that are intentionally unavailable on the static-safe facade.
 *
 * Astryx's dynamic layout props are useful in general, but they can produce
 * runtime CSS custom properties or inline declarations. The primitives in
 * this directory keep the CSP boundary explicit by accepting only compiled
 * classes and finite variants.
 */
export type NoRuntimeStyleProps = {
  readonly style?: never
  readonly xstyle?: never
  readonly width?: never
  readonly height?: never
  readonly maxWidth?: never
  readonly minHeight?: never
  readonly contentWidth?: never
  readonly rowHeight?: never
}

const UNSAFE_RUNTIME_PROP_KEYS = [
  "style",
  "xstyle",
  "width",
  "height",
  "maxWidth",
  "minHeight",
  "contentWidth",
  "rowHeight",
] as const

/**
 * Runtime companion to {@link NoRuntimeStyleProps}.
 *
 * TypeScript catches normal call sites, while this small boundary guard also
 * protects JS consumers and spread objects from forwarding a style escape
 * hatch to the DOM.
 */
export function guardNoRuntimeStyleProps<Props extends Record<string, unknown>>(
  props: Props,
): Omit<Props, keyof NoRuntimeStyleProps> {
  const safeProps = {...props}
  for (const key of UNSAFE_RUNTIME_PROP_KEYS) {
    delete safeProps[key as keyof Props]
  }
  return safeProps as Omit<Props, keyof NoRuntimeStyleProps>
}

/** Alias for callers that prefer an explicit strip verb. */
export const stripNoRuntimeStyleProps = guardNoRuntimeStyleProps

function staticFacade<SafeProps extends object, CoreProps extends object>(
  component: ComponentType<CoreProps>,
): ComponentType<SafeProps> {
  return ((props: SafeProps) =>
    createElement(
      component,
      guardNoRuntimeStyleProps(props as Record<string, unknown>) as unknown as CoreProps,
    )) as ComponentType<SafeProps>
}

/** Remove runtime sizing and styling props while preserving the core API. */
export type StaticCoreProps<Props extends object> = Omit<
  Props,
  keyof NoRuntimeStyleProps
> & NoRuntimeStyleProps

export type SafeCardProps = StaticCoreProps<CardProps>
export type SafeHStackProps = StaticCoreProps<HStackProps>
export type SafeLayoutProps = StaticCoreProps<LayoutProps>
export type SafeLayoutContentProps = StaticCoreProps<LayoutContentProps>
export type SafeLayoutFooterProps = StaticCoreProps<LayoutFooterProps>
export type SafeLayoutHeaderProps = StaticCoreProps<LayoutHeaderProps>
export type SafeLayoutPanelProps = StaticCoreProps<LayoutPanelProps>
export type SafeMetadataListProps = StaticCoreProps<MetadataListProps>
export type SafeMetadataListItemProps = StaticCoreProps<MetadataListItemProps>
export type SafeSectionProps = StaticCoreProps<SectionProps>
export type SafeStackProps = StaticCoreProps<StackProps>
export type SafeVStackProps = StaticCoreProps<VStackProps>

/**
 * Static-safe aliases for Astryx compositions.
 *
 * These facades do not fork Astryx. Their companion prop types remove the
 * runtime-style escape hatches, and the tiny adapter strips them at runtime
 * for JavaScript/spread-object callers as well.
 */
export const SafeCard = staticFacade<SafeCardProps, CardProps>(AstryxCard)
export const StaticCard = SafeCard
export const SafeHStack = staticFacade<SafeHStackProps, HStackProps>(AstryxHStack)
export const StaticHStack = SafeHStack
export const SafeLayout = staticFacade<SafeLayoutProps, LayoutProps>(AstryxLayout)
export const StaticLayout = SafeLayout
export const SafeLayoutContent = staticFacade<SafeLayoutContentProps, LayoutContentProps>(AstryxLayoutContent)
export const StaticLayoutContent = SafeLayoutContent
export const SafeLayoutFooter = staticFacade<SafeLayoutFooterProps, LayoutFooterProps>(AstryxLayoutFooter)
export const StaticLayoutFooter = SafeLayoutFooter
export const SafeLayoutHeader = staticFacade<SafeLayoutHeaderProps, LayoutHeaderProps>(AstryxLayoutHeader)
export const StaticLayoutHeader = SafeLayoutHeader
export const SafeLayoutPanel = staticFacade<SafeLayoutPanelProps, LayoutPanelProps>(AstryxLayoutPanel)
export const StaticLayoutPanel = SafeLayoutPanel
export const SafeMetadataList = staticFacade<SafeMetadataListProps, MetadataListProps>(AstryxMetadataList)
export const StaticMetadataList = SafeMetadataList
export const SafeMetadataListItem = staticFacade<SafeMetadataListItemProps, MetadataListItemProps>(AstryxMetadataListItem)
export const StaticMetadataListItem = SafeMetadataListItem
export const SafeSection = staticFacade<SafeSectionProps, SectionProps>(AstryxSection)
export const StaticSection = SafeSection
export const SafeStack = staticFacade<SafeStackProps, StackProps>(AstryxStack)
export const StaticStack = SafeStack
export const SafeVStack = staticFacade<SafeVStackProps, VStackProps>(AstryxVStack)
export const StaticVStack = SafeVStack
