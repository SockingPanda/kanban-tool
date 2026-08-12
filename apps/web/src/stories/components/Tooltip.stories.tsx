import { useState } from "react"
import type { Meta, StoryObj } from "@storybook/react-vite"

import { Button } from "@astryxdesign/core/Button"
import { Card } from "@astryxdesign/core/Card"
import { Grid } from "@astryxdesign/core/Grid"
import { HStack } from "@astryxdesign/core/HStack"
import { Icon } from "@astryxdesign/core/Icon"
import { IconButton } from "@astryxdesign/core/IconButton"
import { Text } from "@astryxdesign/core/Text"
import { Tooltip, type TooltipProps } from "@astryxdesign/core/Tooltip"
import { VStack } from "@astryxdesign/core/VStack"

import { CatalogPage, CatalogSection, CliEvidence } from "./AstryxCatalog"

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
    layout: "fullscreen",
    astryx: ASTRYX_EVIDENCE,
    docs: {
      description: {
        component: [
          "Astryx Tooltip 的项目目录，覆盖位置、键盘触发、长内容和禁用矩阵。",
          "实现与使用边界来自 `astryx component Tooltip`、`astryx component Tooltip --props` 和 `astryx docs layout`。",
          "页面只使用 Astryx primitives，不引入 story-local CSS、inline style 或手写 SVG。",
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
    hasHoverIndication: { control: "inline-radio", options: ["auto", true, false] },
    onOpenChange: { action: "open changed" },
  },
  args: {
    content: "Shows a short explanation",
    placement: "above",
    alignment: "center",
    delay: 120,
    hideDelay: 0,
    focusTrigger: "auto",
    isEnabled: true,
    hasHoverIndication: "auto",
  },
} satisfies Meta<typeof Tooltip>

export default meta
type Story = StoryObj<typeof meta>

function TooltipTrigger({
  label,
  tooltipProps,
  disabled = false,
}: {
  readonly label: string
  readonly tooltipProps?: Partial<TooltipProps>
  readonly disabled?: boolean
}) {
  return (
    <Tooltip content="Shows a short explanation" {...tooltipProps}>
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
    <CatalogPage title={copy.title} description={copy.description}>
      <CatalogSection title={copy.placements} description={english ? "Placement follows the available edge." : "位置跟随可用边界。"}>
        <Grid columns={{ minWidth: 180, repeat: "fit" }} gap={3}>
          {placements.map(([placement, label]) => (
            <Card variant="muted" padding={4} key={placement}>
              <VStack gap={3}>
                <Text type="code">{placement}</Text>
                <TooltipTrigger label={label} tooltipProps={{ placement }} />
              </VStack>
            </Card>
          ))}
        </Grid>
      </CatalogSection>

      <CatalogSection title={copy.keyboard} description={copy.keyboardDetail}>
        <Card variant="muted" padding={5}>
          <HStack hAlign="center">
            <TooltipTrigger label={english ? "Focus me" : "聚焦我"} tooltipProps={{ focusTrigger: "always", content: copy.keyboardDetail, hasHoverIndication: true }} />
          </HStack>
        </Card>
      </CatalogSection>

      <CatalogSection title={copy.long} description={copy.longDetail}>
        <Card variant="muted" padding={5}>
          <HStack hAlign="center">
            <TooltipTrigger label={english ? "Read details" : "查看详情"} tooltipProps={{ content: copy.longDetail.repeat(2), placement: "below", alignment: "start" }} />
          </HStack>
        </Card>
      </CatalogSection>

      <CatalogSection title={copy.disabled} description={copy.disabledDetail}>
        <Card variant="muted" padding={5}>
          <HStack hAlign="center">
            <TooltipTrigger label={english ? "Unavailable" : "不可用"} disabled tooltipProps={{ content: copy.disabledDetail }} />
          </HStack>
        </Card>
      </CatalogSection>

      <CatalogSection title={copy.toggle} description={enabled ? copy.enabled : copy.disabledState}>
        <Card variant="muted" padding={5}>
          <HStack gap={3} hAlign="center" wrap="wrap">
            <TooltipTrigger label={enabled ? copy.enabled : copy.disabledState} tooltipProps={{ isEnabled: enabled, content: enabled ? copy.enabled : copy.disabledState }} />
            <Button
              type="button"
              label={enabled ? (english ? "Disable tooltip" : "禁用 Tooltip") : (english ? "Enable tooltip" : "启用 Tooltip")}
              variant="ghost"
              onClick={() => setEnabled((current) => !current)}
            />
          </HStack>
        </Card>
      </CatalogSection>

      <CliEvidence evidence={ASTRYX_EVIDENCE} />
    </CatalogPage>
  )
}

export const Default: Story = {
  render: (args) => (
    <CatalogPage title="Tooltip" description="短的补充说明通过 hover 或 focus 出现，不承载操作或必须信息。">
      <CatalogSection title="Playground" description="Controls 直接驱动 Astryx Tooltip 的公开 props。">
        <Card variant="muted" padding={6}>
          <HStack hAlign="center">
            <Tooltip {...args}>
              <Button type="button" label="Hover or focus" variant="secondary" />
            </Tooltip>
          </HStack>
        </Card>
      </CatalogSection>
      <CliEvidence evidence={ASTRYX_EVIDENCE} />
    </CatalogPage>
  ),
}

export const Placements: Story = {
  render: (_, context) => <Matrix english={context.globals.locale === "en"} />,
  parameters: { controls: { exclude: ["children", "onOpenChange"] } },
}

export const Keyboard: Story = {
  args: { focusTrigger: "always", content: "Focus reveals this tooltip", hasHoverIndication: true },
  render: (args) => (
    <CatalogPage title="Tooltip keyboard trigger" description="focusTrigger=always 让键盘用户通过 Tab 发现补充说明。">
      <CatalogSection title="Focus reveals the tooltip" description="先用 Tab 聚焦，再观察 Tooltip 的出现与消失。">
        <Card variant="muted" padding={6}>
          <HStack hAlign="center">
            <Tooltip {...args}>
              <Button type="button" label="Tab to me" variant="secondary" />
            </Tooltip>
          </HStack>
        </Card>
      </CatalogSection>
      <CliEvidence evidence={ASTRYX_EVIDENCE} />
    </CatalogPage>
  ),
}

export const LongContent: Story = {
  args: { content: "A longer explanation stays supplemental and wraps within the layer without becoming an interaction surface.", placement: "below", alignment: "start" },
  render: (args) => (
    <CatalogPage title="Tooltip long content" description="长文本仍然是补充说明；需要操作或复杂结构时应升级为 HoverCard 或 Popover。">
      <CatalogSection title="Bounded supplemental content" description="位置与触发器保持稳定，内容在 layer 内换行。">
        <Card variant="muted" padding={6}>
          <HStack hAlign="center">
            <Tooltip {...args}>
              <Button type="button" label="Read details" variant="secondary" />
            </Tooltip>
          </HStack>
        </Card>
      </CatalogSection>
      <CliEvidence evidence={ASTRYX_EVIDENCE} />
    </CatalogPage>
  ),
}

export const Disabled: Story = {
  args: { isEnabled: false, content: "This tooltip is intentionally disabled" },
  render: (args) => (
    <CatalogPage title="Tooltip disabled" description="通过 isEnabled=false 明确关闭触发器，不把禁用状态误读为内容缺失。">
      <CatalogSection title="Disabled trigger" description="禁用控件保留自身语义；Tooltip 不再响应 hover 或 focus。">
        <Card variant="muted" padding={6}>
          <HStack hAlign="center">
            <Tooltip {...args}>
              <Button type="button" label="No tooltip" variant="secondary" />
            </Tooltip>
          </HStack>
        </Card>
      </CatalogSection>
      <CliEvidence evidence={ASTRYX_EVIDENCE} />
    </CatalogPage>
  ),
}

export const IconOnlyContract: Story = {
  render: (args) => (
    <CatalogPage title="Tooltip for icon-only actions" description="IconButton 提供 accessible label；Tooltip 为视力正常用户补充可见上下文。">
      <CatalogSection title="IconButton + Tooltip" description="两层 contract 分工明确：label 是可访问名称，Tooltip 是视觉补充。">
        <Card variant="muted" padding={6}>
          <HStack hAlign="center">
            <Tooltip {...args} content="搜索任务">
              <IconButton label="搜索任务" icon={<Icon icon="search" />} variant="ghost" />
            </Tooltip>
          </HStack>
        </Card>
      </CatalogSection>
      <CliEvidence evidence={ASTRYX_EVIDENCE} />
    </CatalogPage>
  ),
}

export const All: Story = {
  render: (_, context) => <Matrix english={context.globals.locale === "en"} />,
  parameters: { controls: { exclude: ["children", "onOpenChange"] } },
}
