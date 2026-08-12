import { useState } from "react"
import type { ReactNode } from "react"
import type { Meta, StoryObj } from "@storybook/react-vite"

import { Button } from "@astryxdesign/core/Button"
import { Card } from "@astryxdesign/core/Card"
import { HStack } from "@astryxdesign/core/HStack"
import { Icon } from "@astryxdesign/core/Icon"
import { IconButton } from "@astryxdesign/core/IconButton"
import { Text } from "@astryxdesign/core/Text"
import { VStack } from "@astryxdesign/core/VStack"

import { Grid } from "../../ui/astryx/primitives/Grid"
import { Tooltip, type TooltipProps } from "../../ui/astryx/overlays/Tooltip"

const ASTRYX_EVIDENCE = {
  package: "@astryxdesign/core@0.3.0",
  importPath: "@astryxdesign/core/Tooltip",
  commands: [
    "astryx component Tooltip",
    "astryx component Tooltip --props",
    "astryx component IconButton --props",
    "astryx docs layout",
  ],
  decisions: [
    "Tooltip content stays concise and supplemental; interactive content belongs in HoverCard or Popover.",
    "Hover and focus both remain discoverable, with focusTrigger made explicit for keyboard-critical examples.",
    "Logical placement uses above, below, start, and end so RTL direction can resolve without story-local CSS.",
    "Icon-only actions keep a visible tooltip while IconButton supplies the accessible label contract.",
  ],
} as const

const meta = {
  title: "Components/Tooltip",
  component: Tooltip,
  tags: ["autodocs", "astryx", "cli-verified"],
  parameters: {
    layout: "padded",
    astryx: ASTRYX_EVIDENCE,
    docs: {
      description: {
        component: [
          "Astryx Tooltip 的 strict-CSP safe overlay：短的补充说明通过 hover 或 focus 出现，不承载操作或必须信息。",
          "组件级说明来自 `astryx component Tooltip`、`astryx component Tooltip --props` 与 `astryx docs layout`；story 只提供本地触发器和 copy。",
          "Canvas 保留 controls、placement、hover、focus、disabled 与 icon-only contract，不注入自定义 CatalogPage 或视觉 docs chrome。",
        ].join("\n\n"),
      },
    },
  },
  argTypes: {
    content: { control: "text", description: "提示内容，通常是短的非交互文本。" },
    placement: { control: "inline-radio", options: ["above", "below", "start", "end"] },
    alignment: { control: "inline-radio", options: ["start", "center", "end"] },
    delay: { control: { type: "number", min: 0, step: 50 } },
    hideDelay: { control: { type: "number", min: 0, step: 50 } },
    focusTrigger: { control: "inline-radio", options: ["auto", "always", "never"] },
    isEnabled: { control: "boolean" },
    isOpen: { control: "boolean" },
    isDefaultOpen: { control: "boolean" },
    onOpenChange: { action: "open changed" },
  },
  args: {
    content: "显示简短说明",
    placement: "above",
    alignment: "center",
    delay: 120,
    hideDelay: 0,
    focusTrigger: "auto",
    isEnabled: true,
  },
} satisfies Meta<typeof Tooltip>

export default meta
type Story = StoryObj<typeof meta>

function TooltipTrigger({
  label,
  content,
  tooltipProps,
  disabled = false,
}: {
  readonly label: string
  readonly content: ReactNode
  readonly tooltipProps?: Partial<TooltipProps>
  readonly disabled?: boolean
}) {
  return (
    <Tooltip content={content} {...tooltipProps}>
      <Button type="button" label={label} variant="secondary" isDisabled={disabled} />
    </Tooltip>
  )
}

function Matrix({ english = false }: { readonly english?: boolean }) {
  const copy = english
    ? {
        title: "Tooltip matrix",
        description: "Hover or focus each trigger. The content stays supplemental and keyboard discoverable.",
        placements: "Placements",
        above: "Above",
        below: "Below",
        start: "Start",
        end: "End",
        keyboard: "Keyboard",
        keyboardDetail: "Tab to the trigger to reveal the tooltip without a pointer.",
        long: "Long content",
        longDetail: "A long explanation wraps inside a bounded layer while the trigger remains stable.",
        disabled: "Disabled",
        disabledDetail: "Disabled controls do not emit pointer events; keep the reason on the control itself or use an enabled wrapper.",
        toggle: "Toggle enabled",
        enabled: "Tooltip is enabled",
        disabledState: "Tooltip is disabled",
      }
    : {
        title: "Tooltip 矩阵",
        description: "悬停或聚焦每个触发器；内容只是补充说明，并且可通过键盘发现。",
        placements: "位置",
        above: "上方",
        below: "下方",
        start: "起始侧",
        end: "结束侧",
        keyboard: "键盘",
        keyboardDetail: "Tab 到触发器即可在没有指针时显示 Tooltip。",
        long: "长内容",
        longDetail: "长说明会在有边界的 layer 中换行，触发器保持稳定。",
        disabled: "禁用",
        disabledDetail: "禁用控件不会发出指针事件；原因应留在控件本身，或使用可用的 wrapper。",
        toggle: "切换启用状态",
        enabled: "Tooltip 已启用",
        disabledState: "Tooltip 已禁用",
      }
  const [enabled, setEnabled] = useState(true)
  const placements = [
    ["above", copy.above],
    ["below", copy.below],
    ["start", copy.start],
    ["end", copy.end],
  ] as const

  return (
    <VStack gap={4}>
      <VStack gap={1}>
        <Text as="p" type="supporting">{copy.description}</Text>
      </VStack>
      <Grid label={copy.placements} columns="auto-sm" gap={3}>
        {placements.map(([placement, label]) => (
          <Card variant="muted" padding={4} key={placement}>
            <VStack gap={3}>
              <Text type="code">{placement}</Text>
              <TooltipTrigger label={label} content={label} tooltipProps={{ placement }} />
            </VStack>
          </Card>
        ))}
      </Grid>

      <Card variant="muted" padding={5}>
        <VStack gap={3}>
          <Text weight="semibold">{copy.keyboard}</Text>
          <Text type="supporting">{copy.keyboardDetail}</Text>
          <HStack hAlign="center">
            <TooltipTrigger label={english ? "Focus me" : "聚焦我"} content={copy.keyboardDetail} tooltipProps={{ focusTrigger: "always" }} />
          </HStack>
        </VStack>
      </Card>

      <Card variant="muted" padding={5}>
        <VStack gap={3}>
          <Text weight="semibold">{copy.long}</Text>
          <Text type="supporting">{copy.longDetail}</Text>
          <HStack hAlign="center">
            <TooltipTrigger label={english ? "Read details" : "查看详情"} content={copy.longDetail.repeat(2)} tooltipProps={{ placement: "below", alignment: "start" }} />
          </HStack>
        </VStack>
      </Card>

      <Card variant="muted" padding={5}>
        <VStack gap={3}>
          <Text weight="semibold">{copy.disabled}</Text>
          <Text type="supporting">{copy.disabledDetail}</Text>
          <HStack hAlign="center">
            <TooltipTrigger label={english ? "Unavailable" : "不可用"} content={copy.disabledDetail} disabled tooltipProps={{ isEnabled: false }} />
          </HStack>
        </VStack>
      </Card>

      <Card variant="muted" padding={5}>
        <VStack gap={3}>
          <Text weight="semibold">{copy.toggle}</Text>
          <Text type="supporting">{enabled ? copy.enabled : copy.disabledState}</Text>
          <HStack gap={3} hAlign="center" wrap="wrap">
            <TooltipTrigger
              label={enabled ? copy.enabled : copy.disabledState}
              content={enabled ? copy.enabled : copy.disabledState}
              tooltipProps={{ isEnabled: enabled }}
            />
            <Button
              type="button"
              label={enabled ? (english ? "Disable tooltip" : "禁用 Tooltip") : (english ? "Enable tooltip" : "启用 Tooltip")}
              variant="ghost"
              onClick={() => setEnabled((current) => !current)}
            />
          </HStack>
        </VStack>
      </Card>
    </VStack>
  )
}

export const Default: Story = {
  render: (args) => (
    <Tooltip {...args}>
      <Button type="button" label="Hover or focus" variant="secondary" />
    </Tooltip>
  ),
}

export const Placements: Story = {
  render: (_, context) => <Matrix english={context.globals.locale === "en"} />,
  parameters: { controls: { exclude: ["children", "onOpenChange"] } },
}

export const Keyboard: Story = {
  args: { focusTrigger: "always", content: "Focus reveals this tooltip" },
  render: (args) => (
    <Tooltip {...args}>
      <Button type="button" label="Tab to me" variant="secondary" />
    </Tooltip>
  ),
}

export const LongContent: Story = {
  args: { content: "A longer explanation stays supplemental and wraps within the layer without becoming an interaction surface.", placement: "below", alignment: "start" },
  render: (args) => (
    <Tooltip {...args}>
      <Button type="button" label="Read details" variant="secondary" />
    </Tooltip>
  ),
}

export const Disabled: Story = {
  args: { isEnabled: false, content: "This tooltip is intentionally disabled" },
  render: (args) => (
    <Tooltip {...args}>
      <Button type="button" label="No tooltip" variant="secondary" isDisabled />
    </Tooltip>
  ),
}

export const IconOnlyContract: Story = {
  render: (args) => (
    <Tooltip {...args} content="搜索任务">
      <IconButton label="搜索任务" icon={<Icon icon="search" />} variant="ghost" />
    </Tooltip>
  ),
}

export const All: Story = {
  render: (_, context) => <Matrix english={context.globals.locale === "en"} />,
  parameters: { controls: { exclude: ["children", "onOpenChange"] } },
}
