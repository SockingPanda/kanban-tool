import { useState } from "react"
import type { Meta, StoryObj } from "@storybook/react-vite"

import { Card } from "@astryxdesign/core/Card"
import { Grid } from "../../ui/astryx/primitives/Grid"
import { Heading } from "@astryxdesign/core/Heading"
import { Text } from "@astryxdesign/core/Text"
import { VStack } from "@astryxdesign/core/VStack"

import { TextInput, type TextInputProps, type TextInputType } from "../../ui/astryx/fields/TextInput"

const ASTRYX_EVIDENCE = {
  package: "@astryxdesign/core@0.3.0",
  importPath: "@astryxdesign/core/TextInput",
  commands: [
    "astryx component TextInput",
    "astryx component Field --props",
    "astryx search \"form field validation text input\" --limit 12",
    "astryx docs layout",
  ],
  decisions: [
    "TextInput 已经提供 label、description、required/optional、status 与 disabledMessage；不再额外包一层 Field。",
    "短文本输入必须保留可见 label；isLabelHidden 只用于周围语境已经明确的搜索输入。",
    "错误、警告和成功状态都携带解释性 message；local field 只保留 strict-CSP 可验证的 attached status。",
    "Story 使用 local fields/TextInput，Autodocs Canvas 直接呈现受控字段，不再包裹 CatalogPage、CatalogSection 或 CLI 视觉 chrome。",
  ],
} as const

const meta = {
  title: "Components/TextInput",
  component: TextInput,
  tags: ["autodocs", "astryx", "cli-verified"],
  parameters: {
    layout: "padded",
    astryx: ASTRYX_EVIDENCE,
    docs: {
      description: {
        component: [
          "Astryx TextInput 的 strict-CSP safe field：短文本、搜索、密码与校验状态保持同一字段语法。",
          "组件级说明来自 `astryx component TextInput`、`astryx component Field --props` 与 `astryx docs layout`；story 仅提供确定性的本地 fixture。",
          "Canvas 直接展示 controls、受控值、required/optional、loading、disabled、status 和 clear affordance，不注入自定义目录壳层。",
        ].join("\n\n"),
      },
    },
  },
  argTypes: {
    label: { control: "text", description: "输入字段的可访问标签；默认始终渲染。" },
    value: { control: "text", description: "受控输入值。" },
    type: { control: "select", options: ["text", "password", "email", "search"] satisfies TextInputType[] },
    isRequired: { control: "boolean" },
    isOptional: { control: "boolean" },
    requiredText: { control: "text" },
    optionalText: { control: "text" },
    isDisabled: { control: "boolean" },
    disabledMessage: { control: "text" },
    isLoading: { control: "boolean" },
    hasClear: { control: "boolean" },
    clearLabel: { control: "text" },
    clearText: { control: "text" },
    hasAutoFocus: { control: "boolean" },
    isLabelHidden: { control: "boolean" },
    status: { control: "object" },
    statusVariant: { control: "inline-radio", options: ["attached"] },
    description: { control: "text" },
    placeholder: { control: "text" },
    htmlName: { control: "text" },
    maxLength: { control: "number" },
    autoComplete: { control: "text" },
    onChange: { action: "changed" },
    onEnter: { action: "enter pressed" },
    onKeyDown: { action: "key down" },
    onKeyUp: { action: "key up" },
  },
  args: {
    label: "项目名称",
    value: "看板工具",
    type: "text",
    placeholder: "输入项目名称",
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
        title: "Text input states",
        description: "The same field grammar stays legible across intent, recovery, and affordance states.",
        required: "Required project name",
        optional: "Optional note",
        loading: "Loading value",
        disabled: "Unavailable field",
        disabledMessage: "Editing is unavailable in this fixture.",
        warning: "Warning status",
        error: "Error status",
        success: "Success status",
        clear: "Clearable search",
        clearLabel: "Clear search",
        clearText: "Clear",
        password: "Password",
        passwordPlaceholder: "Enter a password",
      }
    : {
        title: "TextInput 状态",
        description: "同一字段语法在意图、恢复和辅助操作之间保持可读与可操作。",
        required: "必填项目名称",
        optional: "可选备注",
        loading: "加载中的值",
        disabled: "不可用字段",
        disabledMessage: "此 fixture 不允许编辑。",
        warning: "警告状态",
        error: "错误状态",
        success: "成功状态",
        clear: "可清除搜索",
        clearLabel: "清除搜索",
        clearText: "清除",
        password: "密码",
        passwordPlaceholder: "输入密码",
      }

  return (
    <VStack gap={4}>
      <VStack gap={1}>
        <Heading level={2}>{copy.title}</Heading>
        <Text as="p" type="supporting">{copy.description}</Text>
      </VStack>
      <Grid label={copy.title} columns="auto-md" gap={3}>
        <Card variant="muted" padding={4}>
          <VStack gap={3}>
            <TextInput label={copy.required} value="board://default" isRequired />
            <TextInput label={copy.optional} value="" placeholder={english ? "Add context" : "补充上下文"} isOptional />
            <TextInput label={copy.loading} value={english ? "Checking…" : "检查中…"} isLoading />
          </VStack>
        </Card>
        <Card variant="muted" padding={4}>
          <VStack gap={3}>
            <TextInput label={copy.disabled} value={english ? "Read only" : "只读"} isDisabled disabledMessage={copy.disabledMessage} />
            <TextInput label={copy.warning} value="draft" status={{ type: "warning", message: english ? "Review before continuing." : "继续前请检查。" }} />
            <TextInput label={copy.error} value="bad ref" status={{ type: "error", message: english ? "Use a canonical board ref." : "请使用 canonical board ref。" }} />
            <TextInput label={copy.success} value="ready" status={{ type: "success", message: english ? "The value is ready." : "此值已就绪。" }} />
          </VStack>
        </Card>
      </Grid>
      <Grid label={english ? "Text input affordances" : "TextInput 辅助操作"} columns="auto-sm" gap={3}>
        <Card variant="muted" padding={4}>
          <VStack gap={2}>
            <Heading level={3}>{copy.clear}</Heading>
            <ControlledTextInput
              label={copy.clear}
              value="query"
              type="search"
              hasClear
              clearLabel={copy.clearLabel}
              clearText={copy.clearText}
            />
          </VStack>
        </Card>
        <Card variant="muted" padding={4}>
          <VStack gap={2}>
            <Heading level={3}>{copy.password}</Heading>
            <TextInput label={copy.password} value="secret-value" type="password" placeholder={copy.passwordPlaceholder} />
          </VStack>
        </Card>
      </Grid>
    </VStack>
  )
}

export const Default: Story = {
  render: (args) => <ControlledTextInput {...args} />,
}

export const States: Story = {
  render: (_, context) => <Matrix locale={localeFor(context.globals.locale)} />,
  parameters: { controls: { exclude: ["onChange", "onEnter", "onKeyDown", "onKeyUp"] } },
}

export const EdgeCases: Story = {
  args: {
    label: "A very long project name that still needs a stable accessible label",
    value: "board://kanban-tool/with-a-long-canonical-reference",
    description: "Long descriptions wrap without changing the field's alignment.",
    hasClear: true,
    clearLabel: "Clear project name",
    clearText: "Clear",
    status: { type: "warning", message: "This value is long but still valid." },
  },
  render: (args) => <ControlledTextInput {...args} />,
}

export const All: Story = {
  render: (_, context) => <Matrix locale={localeFor(context.globals.locale)} />,
  parameters: { controls: { exclude: ["onChange", "onEnter", "onKeyDown", "onKeyUp"] } },
}
