import type { Meta, StoryObj } from "@storybook/react-vite"

import { Card } from "@astryxdesign/core/Card"
import { Grid } from "@astryxdesign/core/Grid"
import { Heading } from "@astryxdesign/core/Heading"
import { HStack } from "@astryxdesign/core/HStack"
import { Icon } from "@astryxdesign/core/Icon"
import { IconButton, type IconButtonProps } from "@astryxdesign/core/IconButton"
import { Text } from "@astryxdesign/core/Text"
import { VStack } from "@astryxdesign/core/VStack"

import { CatalogPage, CatalogSection, CliEvidence } from "./AstryxCatalog"

const ASTRYX_EVIDENCE = {
  package: "@astryxdesign/core@0.3.0",
  importPath: "@astryxdesign/core/IconButton",
  commands: [
    "astryx component IconButton",
    "astryx component Icon --props",
    "astryx docs layout",
  ],
  decisions: [
    "IconButton is reserved for compact actions whose icon is already familiar.",
    "A specific label is mandatory and becomes the accessible name.",
    "Sighted users still need tooltip context; label alone is not enough.",
    "Ghost is the default toolbar treatment; risk and priority still determine the variant.",
  ],
} as const

const meta = {
  title: "Components/IconButton",
  component: IconButton,
  tags: ["autodocs", "astryx", "cli-verified"],
  parameters: {
    layout: "fullscreen",
    astryx: ASTRYX_EVIDENCE,
    docs: {
      description: {
        component: [
          "Astryx IconButton 的项目目录。只用于空间紧凑且图标含义足够稳定的动作。",
          "实现与文案依据来自 `astryx component IconButton` 与 `astryx component Icon --props`。",
          "页面只使用 Astryx primitives；图标使用 Astryx semantic Icon，不维护 story-local SVG 或 CSS。",
        ].join("\n\n"),
      },
    },
  },
  argTypes: {
    label: { control: "text", description: "必填可访问名称，渲染为 aria-label。" },
    variant: { control: "select", options: ["primary", "secondary", "ghost", "destructive"] },
    size: { control: "inline-radio", options: ["sm", "md", "lg"] },
    elevation: { control: "inline-radio", options: ["none", "low", "med", "high"] },
    isDisabled: { control: "boolean" },
    isLoading: { control: "boolean" },
    isInterruptible: { control: "boolean" },
    tooltip: { control: "text" },
    icon: { control: false },
    onClick: { action: "clicked" },
  },
  args: {
    label: "搜索任务",
    tooltip: "搜索任务",
    variant: "ghost",
    size: "md",
    elevation: "none",
    isDisabled: false,
    isLoading: false,
    isInterruptible: false,
    icon: <Icon icon="search" />,
  },
} satisfies Meta<typeof IconButton>

export default meta
type Story = StoryObj<typeof meta>

const ICON_BUTTON_VARIANTS: readonly NonNullable<IconButtonProps["variant"]>[] = ["primary", "secondary", "ghost", "destructive"]

export const Default: Story = {
  render: (args) => (
    <CatalogPage title="IconButton" description="紧凑动作仍需要完整 label、tooltip、焦点环和状态语义。">
      <CatalogSection title="Playground" description="Controls 直接驱动 Astryx IconButton 的公开 props。">
        <Card variant="muted" padding={6}>
          <HStack hAlign="center"><IconButton {...args} /></HStack>
        </Card>
      </CatalogSection>
      <CliEvidence evidence={ASTRYX_EVIDENCE} />
    </CatalogPage>
  ),
}

export const Variants: Story = {
  render: (args) => (
    <CatalogPage title="IconButton variants" description="Icon-only 动作仍按优先级与风险选择变体。">
      <CatalogSection title="All variants" description="图标本身不决定层级；动作语义决定 variant。">
        <Grid columns={{ minWidth: 180, repeat: "fit" }} gap={3}>
          {ICON_BUTTON_VARIANTS.map((variant) => (
            <Card variant="muted" padding={4} key={variant}>
              <VStack gap={3}>
                <Text type="code">{variant}</Text>
                <IconButton
                  {...args}
                  variant={variant}
                  label={variant === "destructive" ? "删除任务" : `${variant} action`}
                  tooltip={variant === "destructive" ? "删除任务" : `${variant} action`}
                  icon={<Icon icon={variant === "destructive" ? "close" : variant === "primary" ? "check" : "wrench"} />}
                />
              </VStack>
            </Card>
          ))}
        </Grid>
      </CatalogSection>
    </CatalogPage>
  ),
}

export const Sizes: Story = {
  render: (args) => (
    <CatalogPage title="IconButton sizes" description="尺寸改变触控目标和密度，不改变名称与 tooltip contract。">
      <CatalogSection title="sm · md · lg">
        <HStack gap={4} wrap="wrap" vAlign="center">
          {(["sm", "md", "lg"] as const).map((size) => (
            <Card variant="muted" padding={4} key={size}>
              <VStack gap={3}>
                <Text type="code">{size}</Text>
                <IconButton {...args} size={size} label={`设置（${size}）`} tooltip={`设置（${size}）`} icon={<Icon icon="wrench" />} />
              </VStack>
            </Card>
          ))}
        </HStack>
      </CatalogSection>
    </CatalogPage>
  ),
}

export const States: Story = {
  render: (args) => (
    <CatalogPage title="IconButton states" description="状态不能只靠色彩；名称、tooltip 与 loading 反馈始终存在。">
      <CatalogSection title="Resting · loading · disabled">
        <Grid columns={{ minWidth: 220, repeat: "fit" }} gap={3}>
          <Card variant="muted" padding={4}>
            <VStack gap={3}>
              <Heading level={3}>Resting</Heading>
              <Text type="supporting">可触发的搜索动作。</Text>
              <IconButton {...args} label="搜索任务" tooltip="搜索任务" icon={<Icon icon="search" />} isLoading={false} isDisabled={false} />
            </VStack>
          </Card>
          <Card variant="muted" padding={4}>
            <VStack gap={3}>
              <Heading level={3}>Loading</Heading>
              <Text type="supporting">异步刷新进行中，阻止重复触发。</Text>
              <IconButton {...args} label="正在刷新" tooltip="正在刷新" icon={<Icon icon="clock" />} isLoading />
            </VStack>
          </Card>
          <Card variant="muted" padding={4}>
            <VStack gap={3}>
              <Heading level={3}>Disabled</Heading>
              <Text type="supporting">不可用时仍保留动作名称。</Text>
              <IconButton {...args} label="通知不可用" tooltip="通知不可用" icon={<Icon icon="info" />} isDisabled />
            </VStack>
          </Card>
        </Grid>
      </CatalogSection>
    </CatalogPage>
  ),
}

export const SemanticIcons: Story = {
  render: (args) => (
    <CatalogPage title="IconButton semantic icons" description="优先使用 Astryx semantic Icon 名称，避免各 story 维护自己的 SVG 语言。">
      <CatalogSection title="Every icon has a label and tooltip">
        <Grid columns={{ minWidth: 150, repeat: "fit" }} gap={3}>
          {(["search", "wrench", "moreHorizontal", "close", "info"] as const).map((icon) => (
            <Card variant="muted" padding={4} key={icon}>
              <VStack gap={3}>
                <IconButton {...args} label={icon} tooltip={icon} icon={<Icon icon={icon} />} />
                <Text type="code">{icon}</Text>
              </VStack>
            </Card>
          ))}
        </Grid>
      </CatalogSection>
    </CatalogPage>
  ),
}
