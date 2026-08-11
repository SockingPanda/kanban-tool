import type { Meta, StoryObj } from "@storybook/react-vite"

import { FoundationFrame } from "../../ui/foundations"
import { Badge, Button, IconButton, Inline, SegmentedControl, Stack, Surface, Text, TextField } from "../../ui/primitives"

import "./primitives.stories.css"

const meta = {
  title: "Primitives/Controls",
  parameters: {
    layout: "fullscreen",
    docs: {
      description: {
        component: "静态 presentational controls。所有状态均为 story fixture，未连接 service 或 persistence。",
      },
    },
  },
} satisfies Meta

export default meta
type Story = StoryObj<typeof meta>

function SearchIcon() {
  return <svg aria-hidden="true" width="16" height="16" viewBox="0 0 16 16" fill="none"><circle cx="7" cy="7" r="4.25" stroke="currentColor" strokeWidth="1.4" /><path d="m10.25 10.25 3 3" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" /></svg>
}

export const States: Story = {
  render: (_, context) => {
    const english = context.globals.locale === "en"
    return (
      <FoundationFrame className="primitiveStory">
        <Surface as="main" tone="surface" padding="comfortable" aria-labelledby="controls-title">
          <Stack gap="loose">
            <header className="storyHeader">
              <Text as="h1" id="controls-title" size="pageTitle">{english ? "Control primitives" : "控件 primitives"}</Text>
              <Text tone="secondary">{english ? "Keyboard-ready controls for a dense local observation surface." : "为高密度本机观察面准备的键盘可达控件。"}</Text>
            </header>

            <section className="controlSection" aria-labelledby="buttons-title">
              <Text as="h2" id="buttons-title" size="title">{english ? "Actions" : "操作"}</Text>
              <Inline gap="default">
                <Button variant="primary">{english ? "Primary" : "主要操作"}</Button>
                <Button variant="secondary">{english ? "Secondary" : "次要操作"}</Button>
                <Button variant="ghost">{english ? "Ghost" : "幽灵按钮"}</Button>
                <Button variant="danger">{english ? "Danger" : "危险操作"}</Button>
                <Button variant="primary" isLoading>{english ? "Loading" : "加载中"}</Button>
                <Button variant="secondary" isDisabled>{english ? "Disabled" : "禁用"}</Button>
                <IconButton aria-label={english ? "Search" : "搜索"}><SearchIcon /></IconButton>
              </Inline>
            </section>

            <section className="controlSection" aria-labelledby="fields-title">
              <Text as="h2" id="fields-title" size="title">{english ? "Fields" : "字段"}</Text>
              <div className="fieldGrid">
                <TextField label={english ? "Project search" : "项目搜索"} placeholder={english ? "Search projects" : "搜索项目"} startAdornment={<SearchIcon />} hint={english ? "Search is local to this story." : "搜索仅作用于此 story。"} />
                <TextField label={english ? "Loading field" : "加载字段"} value={english ? "Refreshing" : "刷新中"} readOnly isLoading />
                <TextField label={english ? "Error field" : "错误字段"} value="board://example" readOnly error={english ? "The fixture value cannot be resolved." : "无法解析此 fixture 值。"} />
                <TextField label={english ? "Disabled field" : "禁用字段"} placeholder={english ? "Unavailable" : "不可用"} disabled hint={english ? "Disabled without losing contrast." : "禁用后仍保留对比度。"} />
              </div>
            </section>

            <section className="controlSection" aria-labelledby="selection-title">
              <Text as="h2" id="selection-title" size="title">{english ? "Selection and status" : "选择与状态"}</Text>
              <Inline justify="between" align="center" gap="loose">
                <SegmentedControl label={english ? "Task view" : "任务视图"} value="Board" options={[{ value: "Board", label: english ? "Board" : "看板" }, { value: "List", label: english ? "List" : "列表" }, { value: "Table", label: english ? "Table" : "表格" }, { value: "Map", label: english ? "Map" : "映射" }, { value: "Timeline", label: english ? "Timeline" : "时间线", disabled: true }]} />
                <Inline gap="tight"><Badge tone="accent">{english ? "Selected" : "已选择"}</Badge><Badge tone="success">{english ? "Ready" : "就绪"}</Badge><Badge tone="warning">{english ? "Stale" : "过期"}</Badge><Badge tone="danger">{english ? "Error" : "错误"}</Badge></Inline>
              </Inline>
            </section>
          </Stack>
        </Surface>
      </FoundationFrame>
    )
  },
}
