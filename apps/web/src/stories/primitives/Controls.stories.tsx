import { useState } from "react"
import type { Meta, StoryObj } from "@storybook/react-vite"

import { Badge } from "@astryxdesign/core/Badge"
import { Button } from "@astryxdesign/core/Button"
import { Card } from "@astryxdesign/core/Card"
import { HStack } from "@astryxdesign/core/HStack"
import { Heading } from "@astryxdesign/core/Heading"
import { IconButton } from "@astryxdesign/core/IconButton"
import { Text } from "@astryxdesign/core/Text"
import { VStack } from "@astryxdesign/core/VStack"

import "./primitives.stories.css"

const meta = {
  title: "Astryx/Controls",
  parameters: {
    layout: "fullscreen",
    docs: {
      description: {
        component: "通用 actions、status、layout 使用 Astryx；TextInput 等 strict-CSP 不适合的 field 仅在 story-local semantic HTML 中演示。",
      },
    },
  },
} satisfies Meta

export default meta
type Story = StoryObj<typeof meta>

function SearchIcon() {
  return (
    <svg aria-hidden="true" width="16" height="16" viewBox="0 0 16 16" fill="none">
      <circle cx="7" cy="7" r="4.25" stroke="currentColor" strokeWidth="1.4" />
      <path d="m10.25 10.25 3 3" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
    </svg>
  )
}

function StoryField({
  id,
  label,
  value,
  placeholder,
  hint,
  error,
  disabled = false,
  loading = false,
  startAdornment = false,
}: {
  readonly id: string
  readonly label: string
  readonly value?: string
  readonly placeholder?: string
  readonly hint?: string
  readonly error?: string
  readonly disabled?: boolean
  readonly loading?: boolean
  readonly startAdornment?: boolean
}) {
  const messageId = error ? `${id}-error` : hint ? `${id}-hint` : undefined
  return (
    <div className="storyField">
      <label htmlFor={id}>{label}</label>
      <div className="fieldFrame" data-invalid={error ? "true" : undefined} data-disabled={disabled ? "true" : undefined}>
        {startAdornment ? <span className="fieldIcon"><SearchIcon /></span> : null}
        {loading ? <span className="fieldSpinner" aria-hidden="true" /> : null}
        <input
          id={id}
          type="text"
          value={value}
          placeholder={placeholder}
          readOnly={value !== undefined}
          disabled={disabled}
          aria-busy={loading || undefined}
          aria-invalid={error ? true : undefined}
          aria-describedby={messageId}
        />
      </div>
      {error ? <small id={messageId} data-error="true">{error}</small> : hint ? <small id={messageId}>{hint}</small> : null}
    </div>
  )
}

function StorySegmented({
  label,
  value,
  onChange,
  options,
}: {
  readonly label: string
  readonly value: string
  readonly onChange: (value: string) => void
  readonly options: readonly { readonly value: string; readonly label: string; readonly disabled?: boolean }[]
}) {
  return (
    <div className="storySegmented" role="radiogroup" aria-label={label}>
      {options.map((option) => (
        <button
          key={option.value}
          type="button"
          role="radio"
          aria-checked={value === option.value}
          disabled={option.disabled}
          onClick={() => onChange(option.value)}
        >
          {option.label}
        </button>
      ))}
    </div>
  )
}

function ControlsStory({ english }: { readonly english: boolean }) {
  const [view, setView] = useState("board")
  const copy = english
    ? {
        title: "Astryx controls",
        description: "Keyboard-ready controls for a dense local observation surface.",
        actions: "Actions",
        fields: "Fields",
        selection: "Selection and status",
        primary: "Primary",
        secondary: "Secondary",
        ghost: "Ghost",
        danger: "Destructive",
        loading: "Loading",
        disabled: "Disabled",
        search: "Search",
        projectSearch: "Project search",
        searchPlaceholder: "Search projects",
        searchHint: "Search is local to this story.",
        loadingField: "Loading field",
        refreshing: "Refreshing",
        errorField: "Error field",
        unresolved: "The fixture value cannot be resolved.",
        disabledField: "Disabled field",
        unavailable: "Unavailable",
        disabledHint: "Disabled without losing contrast.",
        taskView: "Task view",
        board: "Board",
        list: "List",
        table: "Table",
        map: "Map",
        timeline: "Timeline",
        selected: "Selected",
        ready: "Ready",
        stale: "Stale",
        error: "Error",
      }
    : {
        title: "Astryx 控件",
        description: "为高密度本机观察面准备的键盘可达控件。",
        actions: "操作",
        fields: "字段",
        selection: "选择与状态",
        primary: "主要操作",
        secondary: "次要操作",
        ghost: "幽灵",
        danger: "危险操作",
        loading: "加载中",
        disabled: "禁用",
        search: "搜索",
        projectSearch: "项目搜索",
        searchPlaceholder: "搜索项目",
        searchHint: "搜索仅作用于此 story。",
        loadingField: "加载字段",
        refreshing: "刷新中",
        errorField: "错误字段",
        unresolved: "无法解析此 fixture 值。",
        disabledField: "禁用字段",
        unavailable: "不可用",
        disabledHint: "禁用后仍保留对比度。",
        taskView: "任务视图",
        board: "看板",
        list: "列表",
        table: "表格",
        map: "映射",
        timeline: "时间线",
        selected: "已选择",
        ready: "就绪",
        stale: "过期",
        error: "错误",
      }

  return (
    <div className="primitiveStory" data-density="comfortable">
      <main className="primitiveSurface" aria-labelledby="controls-title">
        <Card variant="transparent" padding={6}>
          <VStack gap={6}>
            <header className="storyHeader">
              <Heading level={1} id="controls-title">{copy.title}</Heading>
              <Text as="p" type="body" color="secondary">{copy.description}</Text>
            </header>

            <section className="controlSection" aria-labelledby="buttons-title">
              <Heading level={2} id="buttons-title">{copy.actions}</Heading>
              <HStack gap={3} align="center" wrap="wrap">
                <Button type="button" label={copy.primary} variant="primary" />
                <Button type="button" label={copy.secondary} variant="secondary" />
                <Button type="button" label={copy.ghost} variant="ghost" />
                <Button type="button" label={copy.danger} variant="destructive" />
                <Button
                  type="button"
                  label={copy.loading}
                  variant="primary"
                  isDisabled
                  aria-busy="true"
                  icon={<span className="controlSpinner" aria-hidden="true" />}
                />
                <Button type="button" label={copy.disabled} variant="secondary" isDisabled />
                <IconButton type="button" label={copy.search} variant="ghost" icon={<SearchIcon />} />
              </HStack>
            </section>

            <section className="controlSection" aria-labelledby="fields-title">
              <Heading level={2} id="fields-title">{copy.fields}</Heading>
              <div className="fieldGrid">
                <StoryField id="project-search" label={copy.projectSearch} placeholder={copy.searchPlaceholder} startAdornment hint={copy.searchHint} />
                <StoryField id="loading-field" label={copy.loadingField} value={copy.refreshing} loading />
                <StoryField id="error-field" label={copy.errorField} value="board://example" error={copy.unresolved} />
                <StoryField id="disabled-field" label={copy.disabledField} placeholder={copy.unavailable} disabled hint={copy.disabledHint} />
              </div>
            </section>

            <section className="controlSection" aria-labelledby="selection-title">
              <Heading level={2} id="selection-title">{copy.selection}</Heading>
              <HStack gap={4} align="center" justify="between" wrap="wrap">
                <StorySegmented
                  label={copy.taskView}
                  value={view}
                  onChange={setView}
                  options={[
                    { value: "board", label: copy.board },
                    { value: "list", label: copy.list },
                    { value: "table", label: copy.table },
                    { value: "map", label: copy.map },
                    { value: "timeline", label: copy.timeline, disabled: true },
                  ]}
                />
                <HStack gap={2} align="center" wrap="wrap">
                  <Badge variant="info" label={copy.selected} />
                  <Badge variant="success" label={copy.ready} />
                  <Badge variant="warning" label={copy.stale} />
                  <span className="storyStatusError" role="status">{copy.error}</span>
                </HStack>
              </HStack>
            </section>
          </VStack>
        </Card>
      </main>
    </div>
  )
}

export const States: Story = {
  render: (_, context) => <ControlsStory english={context.globals.locale === "en"} />,
}
