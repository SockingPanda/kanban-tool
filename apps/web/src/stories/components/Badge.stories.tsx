import type { Meta, StoryObj } from "@storybook/react-vite"

import { Badge, type BadgeProps } from "@astryxdesign/core/Badge"
import { Card } from "@astryxdesign/core/Card"
import { HStack } from "@astryxdesign/core/HStack"
import { Icon } from "@astryxdesign/core/Icon"
import { Text } from "@astryxdesign/core/Text"
import { VStack } from "@astryxdesign/core/VStack"

import { Grid } from "../../ui/astryx/primitives/Grid"

import { CatalogPage, CatalogSection, CliEvidence } from "./AstryxCatalog"

const ASTRYX_EVIDENCE = {
  package: "@astryxdesign/core@0.3.0",
  importPath: "@astryxdesign/core/Badge",
  commands: [
    "astryx component Badge",
    "astryx component Badge --props",
    "astryx component Icon --props",
  ],
  decisions: [
    "Badge marks exceptional state or a grouping category; routine metadata stays plain text.",
    "Semantic variants are reserved for system state that needs attention.",
    "Palette variants classify items and do not imply status.",
    "A badge is read-only; actions remain buttons or links.",
  ],
} as const

const meta = {
  title: "Components/Badge",
  component: Badge,
  tags: ["autodocs", "astryx", "cli-verified"],
  parameters: {
    layout: "fullscreen",
    astryx: ASTRYX_EVIDENCE,
    docs: {
      description: {
        component: [
          "Astryx Badge 的项目目录。用于需要注意的状态或分类，不把普通 metadata 全部胶囊化。",
          "实现与使用边界来自 `astryx component Badge` 与 `astryx component Badge --props`。",
          "页面只使用 Astryx primitives；图标来自 Astryx Icon，不维护 story-local SVG、inline style 或 CSS。",
        ].join("\n\n"),
      },
    },
  },
  argTypes: {
    label: { control: "text", description: "一到两个词的状态或分类文案。" },
    variant: {
      control: "select",
      options: ["neutral", "info", "success", "warning", "error", "blue", "cyan", "green", "orange", "pink", "purple", "red", "teal", "yellow"],
    },
    icon: { control: false },
  },
  args: {
    label: "待复核",
    variant: "warning",
  },
} satisfies Meta<typeof Badge>

export default meta
type Story = StoryObj<typeof meta>

const SEMANTIC_VARIANTS: readonly NonNullable<BadgeProps["variant"]>[] = ["neutral", "info", "success", "warning", "error"]
const PALETTE_VARIANTS: readonly NonNullable<BadgeProps["variant"]>[] = ["blue", "cyan", "green", "orange", "pink", "purple", "red", "teal", "yellow"]

export const Default: Story = {
  render: (args) => (
    <CatalogPage title="Badge" description="短、稳定、可扫描的只读信号；它补充正文，不替代正文。">
      <CatalogSection title="Playground" description="Controls 直接驱动 Astryx Badge 的公开 props。">
        <Card variant="muted" padding={6}><HStack hAlign="center"><Badge {...args} /></HStack></Card>
      </CatalogSection>
      <CliEvidence evidence={ASTRYX_EVIDENCE} />
    </CatalogPage>
  ),
}

export const SemanticVariants: Story = {
  render: (args) => (
    <CatalogPage title="Badge semantic variants" description="语义色只标记值得注意的系统状态。">
      <CatalogSection title="neutral · info · success · warning · error" description="文本承担含义，颜色只加速扫描。">
        <Grid label="Badge semantic variants" columns="auto-sm" gap={3}>
          {SEMANTIC_VARIANTS.map((variant) => (
            <Card variant="muted" padding={4} key={variant}>
              <VStack gap={3}>
                <Text type="code">{variant}</Text>
                <Badge {...args} variant={variant} label={variant === "success" ? "已完成" : variant === "error" ? "需要处理" : variant === "warning" ? "待复核" : variant === "info" ? "运行中" : "未分类"} />
              </VStack>
            </Card>
          ))}
        </Grid>
      </CatalogSection>
    </CatalogPage>
  ),
}

export const PaletteVariants: Story = {
  render: (args) => (
    <CatalogPage title="Badge palette variants" description="非语义 palette 只负责项目、类型或领域分类。">
      <CatalogSection title="Named palette" description="分类色不能代替系统状态语义。">
        <Grid label="Badge palette variants" columns="auto-sm" gap={3}>
          {PALETTE_VARIANTS.map((variant) => (
            <Card variant="muted" padding={4} key={variant}>
              <VStack gap={3}><Text type="code">{variant}</Text><Badge {...args} variant={variant} label={variant} /></VStack>
            </Card>
          ))}
        </Grid>
      </CatalogSection>
    </CatalogPage>
  ),
}

export const Content: Story = {
  render: (args) => (
    <CatalogPage title="Badge content" description="Icon 是辅助信号；label 始终承担主要含义。">
      <CatalogSection title="Short label · count · icon" description="日期、时长和解释性文本应使用 Text。">
        <Grid label="Badge content examples" columns="auto-sm" gap={3}>
          <Card variant="muted" padding={4}><VStack gap={3}><Text type="supporting">status + icon</Text><Badge {...args} variant="success" label="已同步" icon={<Icon icon="check" size="sm" />} /></VStack></Card>
          <Card variant="muted" padding={4}><VStack gap={3}><Text type="supporting">numeric context</Text><Badge {...args} variant="info" label={12} /></VStack></Card>
          <Card variant="muted" padding={4}><VStack gap={3}><Text type="supporting">attention</Text><Badge {...args} variant="warning" label="等待中" icon={<Icon icon="warning" size="sm" />} /></VStack></Card>
          <Card variant="muted" padding={4}><VStack gap={3}><Text type="supporting">category</Text><Badge {...args} variant="purple" label="设计系统" /></VStack></Card>
        </Grid>
      </CatalogSection>
    </CatalogPage>
  ),
}

export const BadgeOrText: Story = {
  render: () => (
    <CatalogPage title="Badge or Text" description="把注意力留给例外，不让每一行都变成彩色胶囊。">
      <CatalogSection title="Status is a badge; metadata is text">
        <Grid label="Badge versus text" columns="auto-md" gap={3}>
          <Card variant="muted" padding={4}>
            <VStack gap={3}>
              <Badge variant="error" label="需要处理" icon={<Icon icon="error" size="sm" />} />
              <Text type="supporting">失败会阻断下一步，因此使用高注意力语义。</Text>
            </VStack>
          </Card>
          <Card variant="muted" padding={4}>
            <VStack gap={3}>
              <Text weight="semibold">6 小时窗口 · 12 个事件</Text>
              <Text type="supporting">时长和计数是普通 metadata，保持为正文。</Text>
            </VStack>
          </Card>
        </Grid>
      </CatalogSection>
    </CatalogPage>
  ),
}
