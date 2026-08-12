import type { Meta, StoryObj } from "@storybook/react-vite"

import { Badge } from "@astryxdesign/core/Badge"
import { Button, type ButtonProps } from "@astryxdesign/core/Button"
import { Card } from "@astryxdesign/core/Card"
import { Grid } from "@astryxdesign/core/Grid"
import { Heading } from "@astryxdesign/core/Heading"
import { HStack } from "@astryxdesign/core/HStack"
import { Icon } from "@astryxdesign/core/Icon"
import { Text } from "@astryxdesign/core/Text"
import { VStack } from "@astryxdesign/core/VStack"

import { CatalogPage, CatalogSection, CliEvidence } from "./AstryxCatalog"

const ASTRYX_EVIDENCE = {
  package: "@astryxdesign/core@0.3.0",
  importPath: "@astryxdesign/core/Button",
  commands: [
    "astryx build \"component documentation catalog with variants sizes states and accessibility notes\"",
    "astryx component Button",
    "astryx template ButtonVariants --skeleton",
    "astryx docs layout",
  ],
  decisions: [
    "Button is for actions; navigation remains a link.",
    "A view has one primary action; secondary and ghost carry lower emphasis.",
    "Loading announces progress and prevents duplicate activation unless isInterruptible is explicit.",
    "Dedicated icon-only actions should use IconButton; this story keeps isIconOnly only as Button contract evidence.",
  ],
} as const

const meta = {
  title: "Components/Button",
  component: Button,
  tags: ["autodocs", "astryx", "cli-verified"],
  parameters: {
    layout: "fullscreen",
    astryx: ASTRYX_EVIDENCE,
    docs: {
      description: {
        component: [
          "Astryx Button 的项目目录。用于本地动作，不用于纯导航。",
          "实现与文案依据来自 `astryx component Button`、`astryx template ButtonVariants --skeleton` 和 `astryx docs layout`。",
          "页面本身只使用 Astryx Layout、Section、Stack、Grid、Card、Heading、Text、Icon、Badge 与 Button；不依赖 story-local CSS。",
        ].join("\n\n"),
      },
    },
  },
  argTypes: {
    label: { control: "text", description: "必填可访问名称；普通按钮中也是可见动作文案。" },
    variant: { control: "select", options: ["primary", "secondary", "ghost", "destructive"] },
    size: { control: "inline-radio", options: ["sm", "md", "lg"] },
    elevation: { control: "inline-radio", options: ["none", "low", "med", "high"] },
    isDisabled: { control: "boolean" },
    isLoading: { control: "boolean" },
    isInterruptible: { control: "boolean" },
    isIconOnly: { control: "boolean" },
    width: { control: "text" },
    tooltip: { control: "text" },
    icon: { control: false },
    endContent: { control: false },
    children: { control: false },
    onClick: { action: "clicked" },
  },
  args: {
    label: "保存更改",
    variant: "primary",
    size: "md",
    elevation: "none",
    isDisabled: false,
    isLoading: false,
    isInterruptible: false,
    isIconOnly: false,
  },
} satisfies Meta<typeof Button>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
  render: (args) => (
    <CatalogPage title="Button" description="一个动作的默认入口：明确的动词 label、单一 primary 层级和稳定的 loading/disabled 语义。">
      <CatalogSection title="Playground" description="Controls 直接驱动 Astryx Button 的公开 props。">
        <Card variant="muted" padding={6}>
          <HStack hAlign="center" wrap="wrap" gap={3}>
            <Button {...args} />
          </HStack>
        </Card>
      </CatalogSection>
      <CliEvidence evidence={ASTRYX_EVIDENCE} />
    </CatalogPage>
  ),
}

const BUTTON_VARIANTS: readonly NonNullable<ButtonProps["variant"]>[] = ["primary", "secondary", "ghost", "destructive"]

export const Variants: Story = {
  render: (args) => (
    <CatalogPage title="Button variants" description="变体表达动作优先级；同一视图只保留一个 primary。">
      <CatalogSection title="All variants" description="来自 Astryx ButtonVariants template 的四级动作矩阵。">
        <Grid columns={{ minWidth: 200, repeat: "fit" }} gap={3}>
          {BUTTON_VARIANTS.map((variant) => (
            <Card variant="muted" padding={4} key={variant}>
              <VStack gap={3}>
                <Text type="code">{variant}</Text>
                <Button {...args} variant={variant} label={variant === "destructive" ? "删除任务" : args.label} />
              </VStack>
            </Card>
          ))}
        </Grid>
      </CatalogSection>
      <Text as="p" type="supporting">Destructive 只表达高风险动作；不可逆操作仍需要确认步骤。</Text>
    </CatalogPage>
  ),
}

export const Sizes: Story = {
  render: (args) => (
    <CatalogPage title="Button sizes" description="尺寸改变密度，不改变动作语义。">
      <CatalogSection title="sm · md · lg" description="Icon 和 loading indicator 随尺寸保持 Astryx 节奏。">
        <HStack gap={4} wrap="wrap" vAlign="center">
          {(["sm", "md", "lg"] as const).map((size) => (
            <Card variant="muted" padding={4} key={size}>
              <VStack gap={3}>
                <Text type="code">{size}</Text>
                <Button {...args} size={size} icon={<Icon icon="chevronRight" />} label={size === "lg" ? "继续到下一步" : "继续"} />
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
    <CatalogPage title="Button states" description="状态必须可感知、可读出，并阻止不安全的重复动作。">
      <CatalogSection title="Resting · loading · disabled" description="Loading 包含 live-region 语义；disabled 保留可访问 label。">
        <Grid columns={{ minWidth: 220, repeat: "fit" }} gap={3}>
          <Card variant="muted" padding={4}>
            <VStack gap={3}>
              <Heading level={3}>Resting</Heading>
              <Text type="supporting">动作可以触发。</Text>
              <Button {...args} isLoading={false} isDisabled={false} label="运行任务" icon={<Icon icon="chevronRight" />} />
            </VStack>
          </Card>
          <Card variant="muted" padding={4}>
            <VStack gap={3}>
              <Heading level={3}>Loading</Heading>
              <Text type="supporting">组件宣布进行中并避免重复提交。</Text>
              <Button {...args} isLoading label="正在运行" icon={<Icon icon="clock" />} />
            </VStack>
          </Card>
          <Card variant="muted" padding={4}>
            <VStack gap={3}>
              <Heading level={3}>Disabled</Heading>
              <Text type="supporting">不可用时仍保留动作名称。</Text>
              <Button {...args} isDisabled label="暂不可用" />
            </VStack>
          </Card>
        </Grid>
      </CatalogSection>
    </CatalogPage>
  ),
}

export const Composition: Story = {
  args: {
    label: "将当前筛选保存为默认视图",
    variant: "secondary",
    icon: <Icon icon="check" />,
    endContent: <Badge variant="info" label="⌘S" />,
    width: "100%",
  },
  render: (args) => (
    <CatalogPage title="Button composition" description="长 label、leading icon、endContent 与 full-width 组成真实工作区动作。">
      <CatalogSection title="Long label · icon · end content" description="Icon 是视觉辅助；label 始终承担可访问名称。">
        <Card variant="muted" padding={6} maxWidth="36rem">
          <Button {...args} />
        </Card>
      </CatalogSection>
    </CatalogPage>
  ),
}

export const IconOnlyContract: Story = {
  args: {
    label: "新建任务",
    variant: "ghost",
    isIconOnly: true,
    icon: <Icon icon="copy" />,
    tooltip: "新建任务",
  },
  render: (args) => (
    <CatalogPage title="Button icon-only contract" description="此页只记录 Button 的 isIconOnly contract；生产中的专用 icon action 优先使用 IconButton。">
      <CatalogSection title="Accessible name + tooltip">
        <Card variant="muted" padding={6}>
          <Button {...args} />
        </Card>
      </CatalogSection>
    </CatalogPage>
  ),
}
