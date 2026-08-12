import type { Meta, StoryObj } from "@storybook/react-vite"

import { Badge } from "@astryxdesign/core/Badge"
import { Button } from "@astryxdesign/core/Button"
import { Card, type CardVariant } from "@astryxdesign/core/Card"
import { Grid } from "@astryxdesign/core/Grid"
import { Heading } from "@astryxdesign/core/Heading"
import { HStack } from "@astryxdesign/core/HStack"
import { Layout, LayoutContent, LayoutFooter, LayoutHeader } from "@astryxdesign/core/Layout"
import { Section } from "@astryxdesign/core/Section"
import { Text } from "@astryxdesign/core/Text"
import { VStack } from "@astryxdesign/core/VStack"

import { CatalogPage, CatalogSection, CliEvidence } from "./AstryxCatalog"

const VARIANTS = [
  "default",
  "transparent",
  "muted",
  "blue",
  "cyan",
  "gray",
  "green",
  "orange",
  "pink",
  "purple",
  "red",
  "teal",
  "yellow",
] as const satisfies readonly CardVariant[]

const PADDINGS = [0, 0.5, 1, 1.5, 2, 3, 4, 5, 6, 8, 10] as const
const ELEVATIONS = ["none", "low", "med", "high"] as const

const ASTRYX_EVIDENCE = {
  package: "@astryxdesign/core@0.3.0",
  importPath: "@astryxdesign/core/Card",
  commands: [
    "astryx build \"component documentation catalog with variants sizes states and accessibility notes\"",
    "astryx component Card",
    "astryx component Layout --props",
    "astryx component Grid --props",
    "astryx component Stack --props",
    "astryx component Section --props",
    "astryx template CardShowcase --skeleton",
  ],
  decisions: [
    "Card is a boundary for discrete content; spacing and Stack remain the default grouping tools.",
    "Sibling cards keep a consistent padding step so content aligns across a grid.",
    "Color variants categorize content and do not carry canonical status semantics.",
    "Layout owns header, content, footer rhythm inside a Card; Grid owns responsive comparison layouts.",
    "This story uses Astryx primitives only and keeps the CLI source visible in Storybook.",
  ],
} as const

const COPY = {
  zh: {
    title: "Card",
    intro: "为独立、可比较的内容建立边界；不是默认的页面分组工具。",
    defaultTitle: "默认容器",
    defaultDescription: "标准 surface、hairline border 与 spacing scale；padding 默认值为 4。",
    variants: "变体",
    variantsDescription: "default、transparent、muted 负责语义层级；颜色变体只表达分类，不承担状态。",
    padding: "内边距",
    paddingDescription: "所有 spacing step 保持同一张卡片骨架，便于比较内容 inset。",
    elevation: "层级",
    elevationDescription: "none 保持平面；low、med、high 只在卡片需要脱离周围 surface 时使用。",
    siblings: "兄弟卡片",
    siblingsDescription: "每张卡片代表一个独立对象；保持共同 padding，避免把卡片嵌套成页面骨架。",
    composition: "组合",
    compositionDescription: "Card 作为边界，Layout 负责 header、content、footer 的节奏与分隔。",
    allVariants: "全部变体",
    allVariantsDescription: "完整展示 CardVariant 合约；非语义颜色用于分类，不用作 canonical 状态。",
    fixture: "Storybook fixture · 仅演示渲染，不连接 API、SSE 或 mutation path。",
    example: "示例",
    cardTitle: "运行记录",
    cardDescription: "一张可独立查看、重排或移除的内容卡片。",
    active: "活动",
    recoveryDescription: "卡片内容保持可读，动作留给明确的恢复路径。",
    open: "打开详情",
    save: "保存",
    cancel: "取消",
    variant: "变体",
    paddingLabel: "padding",
    elevationLabel: "elevation",
    header: "运行 #509",
    headerMeta: "Storybook fixture",
    body: "组件目录",
    bodyDescription: "Layout slot 可以在 Card 内提供稳定的标题、内容和操作层次。",
    footer: "局部示例；不代表真实运行状态。",
  },
  en: {
    title: "Card",
    intro: "A boundary for discrete, comparable content—not the default page grouping tool.",
    defaultTitle: "Default container",
    defaultDescription: "Standard surface, hairline border, and spacing scale; padding defaults to 4.",
    variants: "Variants",
    variantsDescription: "default, transparent, and muted carry hierarchy; color variants categorize without carrying status.",
    padding: "Padding",
    paddingDescription: "Every spacing step keeps the same card frame so content inset is easy to compare.",
    elevation: "Elevation",
    elevationDescription: "none stays flat; low, med, and high lift a card only when it must sit above its surface.",
    siblings: "Sibling cards",
    siblingsDescription: "Each card represents one independent object; shared padding aligns the group without nesting cards.",
    composition: "Composition",
    compositionDescription: "Card owns the boundary while Layout supplies header, content, footer rhythm, and dividers.",
    allVariants: "All variants",
    allVariantsDescription: "The complete CardVariant contract; non-semantic colors categorize and do not stand in for canonical status.",
    fixture: "Storybook fixture · rendering only; no API, SSE, or mutation path.",
    example: "Example",
    cardTitle: "Run record",
    cardDescription: "A card that can be inspected, reordered, or removed independently.",
    active: "Active",
    recoveryDescription: "Keep the content readable while actions point to an explicit recovery path.",
    open: "Open details",
    save: "Save",
    cancel: "Cancel",
    variant: "Variant",
    paddingLabel: "padding",
    elevationLabel: "elevation",
    header: "Run #509",
    headerMeta: "Storybook fixture",
    body: "Component catalog",
    bodyDescription: "Layout slots provide a stable title, content, and action hierarchy inside Card.",
    footer: "Local example; not a live run state.",
  },
} as const

type Locale = keyof typeof COPY

function localeFor(value: unknown): Locale {
  return value === "en" ? "en" : "zh"
}

const meta = {
  title: "Components/Card",
  component: Card,
  tags: ["autodocs", "astryx", "cli-verified"],
  parameters: {
    layout: "fullscreen",
    astryx: ASTRYX_EVIDENCE,
    docs: {
      description: {
        component: [
          "Card 是独立内容的有边界容器。优先用 spacing 与 Stack 建立页面层级；只有需要清晰的交互边界、比较或独立操作时才使用 Card。",
          "页面结构与采纳契约来自 `astryx component Card`、`astryx component Layout --props`、`astryx component Grid --props`、`astryx component Stack --props`、`astryx component Section --props` 与 `astryx template CardShowcase --skeleton`。",
          "页面只使用 Astryx Layout、Section、Stack、Grid、Card、Heading、Text、Badge 与 Button；不依赖 story-local CSS。",
        ].join("\n\n"),
      },
    },
  },
  argTypes: {
    variant: {
      control: "select",
      options: VARIANTS,
      description: "背景变体；颜色变体用于分类，不是状态语义。",
    },
    padding: {
      control: "select",
      options: PADDINGS,
      description: "spacing scale 内边距。",
    },
    elevation: {
      control: "inline-radio",
      options: ELEVATIONS,
      description: "none、low、med、high 的 resting shadow 深度。",
    },
  },
} satisfies Meta<typeof Card>

export default meta
type Story = StoryObj<typeof meta>

function CardBody({
  locale,
  title,
  description,
  variant = "default",
  compact = false,
}: {
  readonly locale: Locale
  readonly title?: string
  readonly description?: string
  readonly variant?: CardVariant
  readonly compact?: boolean
}) {
  const copy = COPY[locale]
  const badgeVariant = variant === "green" ? "success" : variant === "red" ? "error" : "neutral"
  return (
    <VStack gap={compact ? 1.5 : 2}>
      <HStack gap={2} align="center" wrap="wrap">
        <Badge variant={badgeVariant} label={variant === "default" ? copy.example : `${copy.variant}: ${variant}`} />
      </HStack>
      <Heading level={compact ? 4 : 3}>{title ?? copy.cardTitle}</Heading>
      <Text as="p" type="body" color="secondary" textWrap="pretty">
        {description ?? copy.cardDescription}
      </Text>
    </VStack>
  )
}

function SpecimenLabel({ label, detail }: { readonly label: string; readonly detail: string }) {
  return (
    <VStack gap={0.5}>
      <Text weight="semibold">{label}</Text>
      <Text type="code" wordBreak="break-word">{detail}</Text>
    </VStack>
  )
}

export const Default: Story = {
  args: {
    variant: "default",
    padding: 4,
    elevation: "none",
  },
  render: (args, context) => {
    const locale = localeFor(context.globals.locale)
    const copy = COPY[locale]
    return (
      <CatalogPage title={copy.title} description={copy.intro}>
        <CatalogSection title={copy.defaultTitle} description={copy.defaultDescription}>
          <VStack gap={2} maxWidth="42rem">
            <Card {...args}>
              <CardBody locale={locale} />
            </Card>
            <SpecimenLabel label={copy.example} detail={'variant="default" padding=4 elevation="none"'} />
          </VStack>
        </CatalogSection>
        <CliEvidence evidence={ASTRYX_EVIDENCE} />
      </CatalogPage>
    )
  },
}

export const Variants: Story = {
  render: (_, context) => {
    const locale = localeFor(context.globals.locale)
    const copy = COPY[locale]
    const variants = ["default", "transparent", "muted"] as const
    return (
      <CatalogPage title={`${copy.title} · ${copy.variants}`} description={copy.intro}>
        <CatalogSection title={copy.variants} description={copy.variantsDescription}>
          <Grid columns={{ minWidth: 220, repeat: "fit" }} gap={3}>
            {variants.map((variant) => (
              <Card key={variant} variant={variant} padding={4}>
                <VStack gap={2}>
                  <CardBody locale={locale} variant={variant} compact />
                  <Text type="supporting">
                    {variant === "transparent"
                      ? locale === "en" ? "No background or visible border." : "无背景，也无可见边框。"
                      : variant === "muted"
                        ? locale === "en" ? "De-emphasised surface." : "弱化的表面层级。"
                        : locale === "en" ? "Standard bordered card." : "标准有边框卡片。"}
                  </Text>
                </VStack>
              </Card>
            ))}
          </Grid>
        </CatalogSection>
        <CliEvidence evidence={ASTRYX_EVIDENCE} />
      </CatalogPage>
    )
  },
}

export const Padding: Story = {
  render: (_, context) => {
    const locale = localeFor(context.globals.locale)
    const copy = COPY[locale]
    return (
      <CatalogPage title={`${copy.title} · ${copy.padding}`} description={copy.intro}>
        <CatalogSection title={copy.padding} description={copy.paddingDescription}>
          <Grid columns={{ minWidth: 200, repeat: "fit" }} gap={3}>
            {PADDINGS.map((padding) => (
              <Card key={padding} variant="default" padding={padding} minHeight={120}>
                <VStack gap={2}>
                  <CardBody locale={locale} compact />
                  <SpecimenLabel label={`${copy.paddingLabel}=${padding}`} detail={locale === "en" ? "Spacing scale step" : "Spacing scale 步长"} />
                </VStack>
              </Card>
            ))}
          </Grid>
        </CatalogSection>
        <CliEvidence evidence={ASTRYX_EVIDENCE} />
      </CatalogPage>
    )
  },
}

export const Elevation: Story = {
  render: (_, context) => {
    const locale = localeFor(context.globals.locale)
    const copy = COPY[locale]
    return (
      <CatalogPage title={`${copy.title} · ${copy.elevation}`} description={copy.intro}>
        <CatalogSection title={copy.elevation} description={copy.elevationDescription}>
          <Grid columns={{ minWidth: 220, repeat: "fit" }} gap={3}>
            {ELEVATIONS.map((elevation) => (
              <Card key={elevation} variant="default" padding={4} elevation={elevation} minHeight={140}>
                <VStack gap={2}>
                  <CardBody locale={locale} compact />
                  <SpecimenLabel
                    label={`${copy.elevationLabel}="${elevation}"`}
                    detail={elevation === "none" ? (locale === "en" ? "Flat surface" : "平面") : (locale === "en" ? "Resting shadow token" : "Resting shadow token")}
                  />
                </VStack>
              </Card>
            ))}
          </Grid>
        </CatalogSection>
        <CliEvidence evidence={ASTRYX_EVIDENCE} />
      </CatalogPage>
    )
  },
}

export const Siblings: Story = {
  render: (_, context) => {
    const locale = localeFor(context.globals.locale)
    const copy = COPY[locale]
    const items = locale === "en"
      ? [
          ["Queue health", "Three independent observations share one inset.", "default"],
          ["Recent run", "A discrete record with an explicit boundary.", "default"],
          ["Recovery", "Keep recovery actions local to the item.", "muted"],
        ]
      : [
          ["队列健康", "三个独立观察项共享同一组 inset。", "default"],
          ["最近运行", "一个有明确边界的离散记录。", "default"],
          ["恢复动作", "把恢复路径留在对象自身。", "muted"],
        ]
    return (
      <CatalogPage title={`${copy.title} · ${copy.siblings}`} description={copy.intro}>
        <CatalogSection title={copy.siblings} description={copy.siblingsDescription}>
          <Grid columns={{ minWidth: 220, repeat: "fit" }} gap={3}>
            {items.map(([title, description, variant]) => (
              <Card key={title} variant={variant as CardVariant} padding={4}>
                <CardBody locale={locale} title={title} description={description} variant={variant as CardVariant} compact />
              </Card>
            ))}
          </Grid>
        </CatalogSection>
        <CliEvidence evidence={ASTRYX_EVIDENCE} />
      </CatalogPage>
    )
  },
}

export const Composition: Story = {
  render: (_, context) => {
    const locale = localeFor(context.globals.locale)
    const copy = COPY[locale]
    return (
      <CatalogPage title={`${copy.title} · ${copy.composition}`} description={copy.intro}>
        <CatalogSection title={copy.composition} description={copy.compositionDescription}>
          <Card variant="default" padding={0} maxWidth="48rem">
            <Layout
              height="auto"
              header={
                <LayoutHeader hasDivider padding={4}>
                  <HStack hAlign="between" align="center" wrap="wrap" gap={3}>
                    <VStack gap={0.5}>
                      <Heading level={3}>{copy.header}</Heading>
                      <Text type="supporting">{copy.headerMeta}</Text>
                    </VStack>
                    <Badge variant="info" label={copy.active} />
                  </HStack>
                </LayoutHeader>
              }
              content={
                <LayoutContent padding={4} isScrollable={false}>
                  <VStack gap={3}>
                    <VStack gap={1}>
                      <Heading level={3}>{copy.body}</Heading>
                      <Text as="p" type="body" color="secondary">{copy.bodyDescription}</Text>
                    </VStack>
                    <Section variant="muted" padding={3}>
                      <HStack gap={2} align="center" wrap="wrap">
                        <Badge variant="success" label={locale === "en" ? "Recovery" : "恢复边界"} />
                        <Text type="supporting">{copy.recoveryDescription}</Text>
                      </HStack>
                    </Section>
                  </VStack>
                </LayoutContent>
              }
              footer={
                <LayoutFooter hasDivider padding={4}>
                  <HStack hAlign="between" align="center" wrap="wrap" gap={3}>
                    <Text type="supporting">{copy.footer}</Text>
                    <HStack gap={2} wrap="wrap">
                      <Button type="button" label={copy.cancel} variant="ghost" />
                      <Button type="button" label={copy.open} variant="primary" />
                    </HStack>
                  </HStack>
                </LayoutFooter>
              }
            />
          </Card>
        </CatalogSection>
        <CliEvidence evidence={ASTRYX_EVIDENCE} />
      </CatalogPage>
    )
  },
}

export const AllVariants: Story = {
  render: (_, context) => {
    const locale = localeFor(context.globals.locale)
    const copy = COPY[locale]
    return (
      <CatalogPage title={`${copy.title} · ${copy.allVariants}`} description={copy.intro}>
        <CatalogSection title={copy.allVariants} description={copy.allVariantsDescription}>
          <Grid columns={{ minWidth: 220, repeat: "fit" }} gap={3}>
            {VARIANTS.map((variant) => (
              <Card key={variant} variant={variant} padding={3} minHeight={132}>
                <CardBody locale={locale} variant={variant} compact />
              </Card>
            ))}
          </Grid>
        </CatalogSection>
        <CliEvidence evidence={ASTRYX_EVIDENCE} />
      </CatalogPage>
    )
  },
}
