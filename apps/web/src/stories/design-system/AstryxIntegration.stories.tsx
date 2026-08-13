import type { Meta, StoryObj } from "@storybook/react-vite"

import { Badge } from "@astryxdesign/core/Badge"
import { Card } from "@astryxdesign/core/Card"
import { Heading } from "@astryxdesign/core/Heading"
import { HStack } from "@astryxdesign/core/HStack"
import { Icon } from "@astryxdesign/core/Icon"
import { Layout } from "@astryxdesign/core/Layout"
import { Text } from "@astryxdesign/core/Text"
import { VStack } from "@astryxdesign/core/VStack"

import { Grid } from "../../ui/astryx/primitives/Grid"

import { CatalogPage, CatalogSection } from "../components/AstryxCatalog"

const ASTRYX_INTEGRATION = {
  versions: {
    core: "0.3.0",
    cli: "0.3.0",
    theme: "src/theme/astryx.ts (extends @astryxdesign/theme-neutral@0.3.0)",
  },
  discovery: [
    "pnpm astryx:manifest",
    "pnpm astryx:search -- \"<intent>\"",
    "pnpm astryx:component -- <Component>",
    "pnpm astryx:templates",
  ],
  delivery: [
    "astryx build \"<page intent>\"",
    "astryx template <name> --skeleton",
    "astryx layout check '<expression>'",
    "astryx layout expand '<expression>' <path>",
  ],
  ownership: [
    "astryx swizzle --list",
    "astryx swizzle <Component> --output <owned-path>",
    "astryx doctor",
    "astryx upgrade --apply",
  ],
} as const

const meta = {
  title: "Design System/Astryx Integration",
  component: Layout,
  tags: ["autodocs", "astryx", "cli-verified"],
  parameters: {
    layout: "fullscreen",
    astryx: ASTRYX_INTEGRATION,
    docs: {
      description: {
        component: [
          "kanban-tool 的 Astryx 集成事实页。它记录当前版本、CLI-first 工作流、受控 swizzle 边界与验证状态。",
          "组件文档以真实 `component` metadata、Controls、states 和 `parameters.astryx` 为可查询资产。",
          "页面本身只组合 Astryx primitives，不依赖 story-local CSS。",
        ].join("\n\n"),
      },
    },
  },
} satisfies Meta<typeof Layout>

export default meta
type Story = StoryObj<typeof meta>

function CommandGroup({ title, description, commands }: {
  readonly title: string
  readonly description: string
  readonly commands: readonly string[]
}) {
  return (
    <Card variant="muted" padding={4}>
      <VStack gap={3}>
        <VStack gap={1}>
          <Heading level={3}>{title}</Heading>
          <Text type="supporting">{description}</Text>
        </VStack>
        {commands.map((command) => <Text key={command} as="p" type="code" wordBreak="break-word">{command}</Text>)}
      </VStack>
    </Card>
  )
}

export const Workflow: Story = {
  render: () => (
    <CatalogPage title="Astryx integration" description="先查询组件与模板，再组合 primitives；只有必须拥有源码时才 swizzle。">
      <CatalogSection title="Installed contract" description="Core、CLI 和 app-owned Astryx theme 版本锁步，Storybook 与产品入口加载同一 built theme。">
        <HStack gap={2} wrap="wrap">
          <Badge variant="info" label={`core ${ASTRYX_INTEGRATION.versions.core}`} />
          <Badge variant="info" label={`cli ${ASTRYX_INTEGRATION.versions.cli}`} />
          <Badge variant="neutral" label="astryx built theme" />
        </HStack>
      </CatalogSection>
      <CatalogSection title="CLI-first delivery loop" description="这些命令构成页面迁移和 Storybook 证据更新的默认顺序。">
        <Grid label="Astryx delivery workflow" columns="auto-md" gap={3}>
          <CommandGroup title="1 · Discover" description="确认组件、props、模板与文档。" commands={ASTRYX_INTEGRATION.discovery} />
          <CommandGroup title="2 · Compose" description="从 intent、template 和 layout grammar 生成可审查骨架。" commands={ASTRYX_INTEGRATION.delivery} />
          <CommandGroup title="3 · Own deliberately" description="swizzle 是源码所有权切换，不是默认安装步骤。" commands={ASTRYX_INTEGRATION.ownership} />
        </Grid>
      </CatalogSection>
      <CatalogSection title="Storybook evidence contract" description="每个 Astryx component story 都必须可由人和 MCP/CLI 同时检索。">
        <Grid label="Storybook evidence contract" columns="auto-md" gap={3}>
          {[
            ["component", "绑定真实 Astryx export，生成 Props/Controls 与 componentPath。"],
            ["parameters.astryx", "记录 package、importPath、执行过的 CLI 命令与采纳契约。"],
            ["states", "展示真实 default、loading、disabled、error 与边界内容。"],
            ["implementation", "优先 Astryx primitives；无直接布局标签、story-local CSS 或自绘 SVG。"],
          ].map(([title, body]) => (
            <Card variant="muted" padding={4} key={title}>
              <VStack gap={2}>
                <Text type="code">{title}</Text>
                <Text type="supporting">{body}</Text>
              </VStack>
            </Card>
          ))}
        </Grid>
      </CatalogSection>
    </CatalogPage>
  ),
}

export const DoctorStatus: Story = {
  render: () => (
    <CatalogPage title="Astryx doctor" description="最近一次本地诊断：6 pass、1 warn、0 fail、1 info。">
      <CatalogSection title="Current evidence" description="Core/CLI 对齐、配置、agent docs 与 peer dependencies 均通过。">
        <Grid label="Astryx doctor status" columns="auto-md" gap={3}>
          <Card variant="muted" padding={4}>
            <VStack gap={3}>
              <HStack gap={2}><Icon icon="success" color="success" /><Heading level={3}>Passing</Heading></HStack>
              <Text type="supporting">Node、core、version alignment、config、agent docs、peer dependencies。</Text>
            </VStack>
          </Card>
          <Card variant="muted" padding={4}>
            <VStack gap={3}>
              <HStack gap={2}><Icon icon="warning" color="warning" /><Heading level={3}>Known warning</Heading></HStack>
              <Text type="supporting">doctor 未识别 theme package；但 package dependency、CSS import 与 Storybook Theme provider 已分别核对存在。此项保持为待上游诊断的 warning，不伪报为 pass。</Text>
            </VStack>
          </Card>
        </Grid>
      </CatalogSection>
      <Text as="p" type="code">pnpm astryx:doctor</Text>
    </CatalogPage>
  ),
}

export const SwizzleBoundary: Story = {
  render: () => (
    <CatalogPage title="Swizzle boundary" description="swizzle 表示项目开始拥有一份组件源码；只有 contract 无法满足需求时使用。">
      <CatalogSection title="Decision gate">
        <Grid label="Astryx swizzle decision" columns="auto-md" gap={3}>
          <CommandGroup
            title="Prefer composition"
            description="先组合公开 props、slots、Layout、Stack、Grid、Section、Card 与 semantic components。"
            commands={["astryx component <Component> --props", "astryx search \"<need>\"", "astryx template <name> --skeleton"]}
          />
          <CommandGroup
            title="Swizzle only when owned"
            description="复制前记录原因、目标路径和与上游升级的差异责任；复制后用 Storybook 固化新 contract。"
            commands={["astryx swizzle --list", "astryx swizzle <Component> --output <owned-path>"]}
          />
        </Grid>
      </CatalogSection>
    </CatalogPage>
  ),
}
