import { useState } from "react"
import type { Meta, StoryObj } from "@storybook/react-vite"

import { Card } from "@astryxdesign/core/Card"
import { FormLayout } from "@astryxdesign/core/FormLayout"
import { Grid } from "@astryxdesign/core/Grid"
import { Heading } from "@astryxdesign/core/Heading"
import { TextInput, type TextInputProps, type TextInputSize } from "@astryxdesign/core/TextInput"
import { Text } from "@astryxdesign/core/Text"
import { VStack } from "@astryxdesign/core/VStack"

import { CatalogPage, CatalogSection, CliEvidence } from "./AstryxCatalog"

const ASTRYX_EVIDENCE = {
  package: "@astryxdesign/core@0.3.0",
  importPath: "@astryxdesign/core/TextInput",
  commands: [
    "astryx component TextInput",
    "astryx component Field --props",
    "astryx component FormLayout --props",
    "astryx search \"form field validation text input\" --limit 12",
    "astryx template FieldShowcase --skeleton",
    "astryx docs layout",
  ],
  decisions: [
    "TextInput 已经提供 label、description、required/optional、status 与 disabledMessage；不再额外包一层 Field。",
    "短文本输入必须保留可见 label；isLabelHidden 只用于周围语境已经明确的搜索输入。",
    "错误、警告和成功状态都携带解释性 message；statusVariant 用于比较 attached、detached 与 tooltip。",
    "FieldShowcase 的骨架采用 Stack/Field/TextInput；本目录使用 Astryx CatalogPage/Section/FormLayout 记录同一字段契约。",
    "页面布局只使用 Astryx primitives，不依赖 story-local CSS；TextInput 的 width 负责整个字段的对齐。",
  ],
} as const

const meta = {
  title: "Components/TextInput",
  component: TextInput,
  tags: ["autodocs", "astryx", "cli-verified"],
  parameters: {
    layout: "fullscreen",
    astryx: ASTRYX_EVIDENCE,
    docs: {
      description: {
        component: [
          "Astryx TextInput 的项目目录：短文本、搜索、密码与校验状态都保持同一字段语法。",
          "实现与内容依据来自 `astryx component TextInput`、`astryx component Field --props`、`astryx component FormLayout --props` 和 `astryx template FieldShowcase --skeleton`。",
          "页面只使用 Astryx CatalogPage、CatalogSection、FormLayout、Grid、Card、VStack、Heading、Text 与 TextInput；不依赖 story-local CSS。",
        ].join("\n\n"),
      },
    },
  },
  argTypes: {
    label: { control: "text", description: "输入字段的可访问标签；默认始终渲染。" },
    value: { control: "text", description: "受控输入值。" },
    type: { control: "select", options: ["text", "password", "email"] },
    size: { control: "inline-radio", options: ["sm", "md", "lg"] satisfies TextInputSize[] },
    isRequired: { control: "boolean" },
    isOptional: { control: "boolean" },
    isDisabled: { control: "boolean" },
    disabledMessage: { control: "text" },
    isLoading: { control: "boolean" },
    hasClear: { control: "boolean" },
    hasAutoFocus: { control: "boolean" },
    isLabelHidden: { control: "boolean" },
    status: { control: "object" },
    statusVariant: { control: "select", options: ["attached", "detached", "tooltip"] },
    description: { control: "text" },
    placeholder: { control: "text" },
    labelTooltip: { control: "text" },
    startIcon: { control: "text" },
    htmlName: { control: "text" },
    width: { control: "text" },
    onChange: { action: "changed" },
    changeAction: { action: "change action" },
    onEnter: { action: "enter pressed" },
    onKeyDown: { action: "key down" },
  },
  args: {
    label: "Project name",
    value: "kanban-tool",
    type: "text",
    size: "md",
    placeholder: "Enter a project name",
    width: "22rem",
    hasClear: false,
    isDisabled: false,
    isLoading: false,
  },
} satisfies Meta<typeof TextInput>

export default meta
type Story = StoryObj<typeof meta>

function ControlledTextInput(props: TextInputProps) {
  const [value, setValue] = useState(props.value)

  return (
    <TextInput
      {...props}
      value={value}
      onChange={(nextValue, event) => {
        setValue(nextValue)
        props.onChange?.(nextValue, event)
      }}
    />
  )
}

type StorybookLocale = "zh" | "en"

function localeFor(value: unknown): StorybookLocale {
  return value === "en" ? "en" : "zh"
}

function Matrix({ locale }: { readonly locale: StorybookLocale }) {
  const english = locale === "en"
  const copy = english
    ? {
        title: "Text input matrix",
        description: "The same field grammar remains legible across size, intent, and recovery states.",
        sizes: "Sizes",
        sizesDescription: "Compare the three contract sizes without changing field semantics.",
        compact: "Compact",
        default: "Default",
        spacious: "Spacious",
        states: "States",
        statesDescription: "Every state keeps its label and explanation.",
        required: "Required project name",
        optional: "Optional note",
        loading: "Loading value",
        disabled: "Unavailable field",
        disabledMessage: "Editing is unavailable in this fixture.",
        warning: "Warning status",
        error: "Error status",
        success: "Success status",
        tooltip: "Tooltip status",
        affordances: "Affordances",
        affordancesDescription: "Clear, search, tooltip, and secure input examples.",
        clear: "Clearable search",
        password: "Password",
        passwordPlaceholder: "••••••••",
      }
    : {
        title: "TextInput 矩阵",
        description: "同一字段语法在尺寸、意图和恢复状态之间保持可读与可操作。",
        sizes: "尺寸",
        sizesDescription: "比较三档 contract 尺寸，不改变字段语义。",
        compact: "紧凑",
        default: "默认",
        spacious: "宽松",
        states: "状态",
        statesDescription: "每个状态都保留标签与原因。",
        required: "必填项目名称",
        optional: "可选备注",
        loading: "加载中的值",
        disabled: "不可用字段",
        disabledMessage: "此 fixture 不允许编辑。",
        warning: "警告状态",
        error: "错误状态",
        success: "成功状态",
        tooltip: "提示状态",
        affordances: "辅助操作",
        affordancesDescription: "清除、搜索、提示和安全输入示例。",
        clear: "可清除搜索",
        password: "密码",
        passwordPlaceholder: "••••••••",
      }

  return (
    <CatalogPage title={copy.title} description={copy.description}>
      <CatalogSection title={copy.sizes} description={copy.sizesDescription}>
        <Grid columns={{ minWidth: 220, repeat: "fit" }} gap={3}>
          <Card variant="muted" padding={4}>
            <VStack gap={2}>
              <Text type="code">sm</Text>
              <TextInput label={copy.compact} value="small" size="sm" width="100%" />
            </VStack>
          </Card>
          <Card variant="muted" padding={4}>
            <VStack gap={2}>
              <Text type="code">md</Text>
              <TextInput label={copy.default} value="medium" size="md" width="100%" />
            </VStack>
          </Card>
          <Card variant="muted" padding={4}>
            <VStack gap={2}>
              <Text type="code">lg</Text>
              <TextInput label={copy.spacious} value="large" size="lg" width="100%" />
            </VStack>
          </Card>
        </Grid>
      </CatalogSection>

      <CatalogSection title={copy.states} description={copy.statesDescription}>
        <FormLayout direction="vertical">
          <TextInput label={copy.required} value="board://default" isRequired width="100%" />
          <TextInput label={copy.optional} value="" placeholder={english ? "Add context" : "补充上下文"} isOptional width="100%" />
          <TextInput label={copy.loading} value={english ? "Checking…" : "检查中…"} isLoading width="100%" />
          <TextInput label={copy.disabled} value={english ? "Read only" : "只读"} isDisabled disabledMessage={copy.disabledMessage} width="100%" />
          <TextInput label={copy.warning} value="draft" status={{ type: "warning", message: english ? "Review before continuing." : "继续前请检查。" }} width="100%" />
          <TextInput label={copy.error} value="bad ref" status={{ type: "error", message: english ? "Use a canonical board ref." : "请使用 canonical board ref。" }} width="100%" />
          <TextInput label={copy.success} value="ready" status={{ type: "success", message: english ? "The value is ready." : "此值已就绪。" }} statusVariant="detached" width="100%" />
          <TextInput label={copy.tooltip} value="needs review" status={{ type: "warning", message: english ? "Tooltip keeps the status message compact." : "提示模式让状态说明保持紧凑。" }} statusVariant="tooltip" width="100%" />
        </FormLayout>
      </CatalogSection>

      <CatalogSection title={copy.affordances} description={copy.affordancesDescription}>
        <Grid columns={{ minWidth: 280, repeat: "fit" }} gap={3}>
          <Card variant="muted" padding={4}>
            <VStack gap={3}>
              <Heading level={3}>{copy.clear}</Heading>
              <ControlledTextInput label={copy.clear} value="query" hasClear startIcon="search" labelTooltip={english ? "Matches local project names." : "匹配本地项目名称。"} width="100%" />
            </VStack>
          </Card>
          <Card variant="muted" padding={4}>
            <VStack gap={3}>
              <Heading level={3}>{copy.password}</Heading>
              <TextInput label={copy.password} value="secret-value" type="password" isLabelHidden placeholder={copy.passwordPlaceholder} width="100%" />
            </VStack>
          </Card>
        </Grid>
      </CatalogSection>

      <CliEvidence evidence={ASTRYX_EVIDENCE} />
    </CatalogPage>
  )
}

export const Default: Story = {
  render: (args) => (
    <CatalogPage title="TextInput" description="短文本字段的默认入口：可见标签、受控值和明确的 placeholder。">
      <CatalogSection title="Playground" description="Controls 直接驱动 Astryx TextInput 的公开 props。">
        <Card variant="muted" padding={6}>
          <FormLayout direction="vertical">
            <ControlledTextInput {...args} />
          </FormLayout>
        </Card>
      </CatalogSection>
      <CliEvidence evidence={ASTRYX_EVIDENCE} />
    </CatalogPage>
  ),
}

export const Sizes: Story = {
  render: (_, context) => <Matrix locale={localeFor(context.globals.locale)} />,
  parameters: { layout: "padded", controls: { exclude: ["onChange", "changeAction", "onEnter", "onKeyDown"] } },
}

export const States: Story = {
  render: (_, context) => <Matrix locale={localeFor(context.globals.locale)} />,
  parameters: { layout: "padded", controls: { exclude: ["onChange", "changeAction", "onEnter", "onKeyDown"] } },
}

export const EdgeCases: Story = {
  args: {
    label: "A very long project name that still needs a stable accessible label",
    value: "board://kanban-tool/with-a-long-canonical-reference",
    description: "Long descriptions wrap without changing the field's alignment.",
    labelTooltip: "The tooltip explains the label without replacing it.",
    hasClear: true,
    startIcon: "search",
    width: "min(100%, 34rem)",
    status: { type: "warning", message: "This value is long but still valid." },
  },
  render: (args) => (
    <CatalogPage title="TextInput edge cases" description="Long labels, descriptions, clear affordances and status messages remain aligned as one field.">
      <CatalogSection title="Long content + recovery affordance" description="The accessible label remains visible while the clear action returns focus to the input.">
        <Card variant="muted" padding={6}>
          <ControlledTextInput {...args} />
        </Card>
      </CatalogSection>
      <CliEvidence evidence={ASTRYX_EVIDENCE} />
    </CatalogPage>
  ),
}

export const All: Story = {
  render: (_, context) => <Matrix locale={localeFor(context.globals.locale)} />,
  parameters: { layout: "padded", controls: { exclude: ["onChange", "changeAction", "onEnter", "onKeyDown"] } },
}
