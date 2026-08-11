import type { Meta, StoryObj } from "@storybook/react-vite"

import { FoundationFrame } from "../../ui/foundations"
import { Badge, Button, Inline, SegmentedControl, StateBoundary, Stack, Surface, Text, TextField } from "../../ui/primitives"

import "./foundations.stories.css"

const meta = {
  title: "Foundations/Environment",
  parameters: {
    layout: "fullscreen",
    docs: {
      description: {
        component: "仅用于 Storybook 组件评审的静态环境；不连接 API、SSE 或 canonical mutation path。",
      },
    },
  },
} satisfies Meta

export default meta
type Story = StoryObj<typeof meta>

export const Baseline: Story = {
  render: (_, context) => {
    const english = context.globals.locale === "en"
    const copy = english
      ? {
          title: "Foundation environment",
          description: "A quiet, dense surface for checking type, theme, focus and resilient states.",
          field: "Project search",
          placeholder: "Search local projects",
          hint: "Story fixture only; no canonical request is made.",
          action: "Keyboard action",
          status: "Fixture status: ready",
          views: ["Board", "List", "Table", "Map"],
        }
      : {
          title: "基础环境",
          description: "用于检查排版、主题、焦点与恢复状态的安静高密度表面。",
          field: "项目搜索",
          placeholder: "搜索本机项目",
          hint: "仅为 Storybook fixture；不会发起 canonical 请求。",
          action: "键盘操作",
          status: "Fixture 状态：就绪",
          views: ["Board", "List", "Table", "Map"],
        }

    return (
      <FoundationFrame className="foundationStory">
        <Surface as="main" tone="surface" padding="comfortable" aria-labelledby="environment-title">
          <Stack gap="loose">
            <header className="storyHeader">
              <Inline gap="tight">
                <Badge tone="accent">Storybook</Badge>
                <Text as="span" size="supporting" tone="secondary">{english ? "Foundations" : "基础层"}</Text>
              </Inline>
              <Text as="h1" id="environment-title" size="pageTitle">{copy.title}</Text>
              <Text tone="secondary">{copy.description}</Text>
            </header>

            <section aria-labelledby="environment-controls-title">
              <Stack gap="default">
                <Text as="h2" id="environment-controls-title" size="title">{english ? "Semantic controls" : "语义控件"}</Text>
                <Inline gap="loose" align="end">
                  <TextField label={copy.field} placeholder={copy.placeholder} hint={copy.hint} />
                  <Button variant="primary">{copy.action}</Button>
                </Inline>
                <SegmentedControl label={english ? "Display view" : "展示视图"} value="Board" options={copy.views.map((label) => ({ value: label, label }))} />
              </Stack>
            </section>

            <section className="storySplit" aria-labelledby="environment-states-title">
              <Stack gap="default">
                <Text as="h2" id="environment-states-title" size="title">{english ? "State boundary" : "状态边界"}</Text>
                <StateBoundary mode="stale" title={english ? "Showing the last known snapshot" : "正在显示最近快照"} description={english ? "Reconnect is available without losing focus." : "可以重新连接，当前焦点不会丢失。"} onRetry={() => undefined} retryLabel={english ? "Reconnect" : "重新连接"} />
              </Stack>
              <Stack gap="default">
                <Text as="h2" size="title">{english ? "Focus and status" : "焦点与状态"}</Text>
                <Surface tone="layer" padding="compact">
                  <Inline justify="between">
                    <Text as="span" size="supporting" tone="secondary">{copy.status}</Text>
                    <Badge tone="success">{english ? "Ready" : "就绪"}</Badge>
                  </Inline>
                </Surface>
              </Stack>
            </section>
          </Stack>
        </Surface>
      </FoundationFrame>
    )
  },
}
