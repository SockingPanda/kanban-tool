import { useState } from "react"
import type { Meta, StoryObj } from "@storybook/react-vite"

import { Badge } from "@astryxdesign/core/Badge"
import { Button } from "@astryxdesign/core/Button"
import { Card } from "@astryxdesign/core/Card"
import { HStack } from "@astryxdesign/core/HStack"
import { Heading } from "@astryxdesign/core/Heading"
import { Text } from "@astryxdesign/core/Text"
import { VStack } from "@astryxdesign/core/VStack"

import "./foundations.stories.css"

const meta = {
  title: "Foundations/Environment",
  parameters: {
    layout: "fullscreen",
    docs: {
      description: {
        component: "静态 Storybook 评审环境；通用控件来自 Astryx，字段和状态是 story-local semantic fallback。",
      },
    },
  },
} satisfies Meta

export default meta
type Story = StoryObj<typeof meta>

function StorySegmented({
  label,
  value,
  onChange,
  options,
}: {
  readonly label: string
  readonly value: string
  readonly onChange: (value: string) => void
  readonly options: readonly { readonly value: string; readonly label: string }[]
}) {
  return (
    <div className="storySegmented" role="radiogroup" aria-label={label}>
      {options.map((option) => (
        <button key={option.value} type="button" role="radio" aria-checked={value === option.value} onClick={() => onChange(option.value)}>
          {option.label}
        </button>
      ))}
    </div>
  )
}

function EnvironmentStory({ english }: { readonly english: boolean }) {
  const [view, setView] = useState("board")
  const copy = english
    ? {
        title: "Foundation environment",
        description: "A quiet, dense surface for checking type, theme, focus and resilient states.",
        field: "Project search",
        placeholder: "Search local projects",
        hint: "Story fixture only; no canonical request is made.",
        action: "Keyboard action",
        status: "Fixture status: ready",
        views: { board: "Board", list: "List", table: "Table", map: "Map" },
        stateTitle: "Showing the last known snapshot",
        stateDescription: "Reconnect is available without losing focus.",
        reconnect: "Reconnect",
      }
    : {
        title: "基础环境",
        description: "用于检查排版、主题、焦点与恢复状态的安静高密度表面。",
        field: "项目搜索",
        placeholder: "搜索本机项目",
        hint: "仅为 Storybook fixture；不会发起 canonical 请求。",
        action: "键盘操作",
        status: "Fixture 状态：就绪",
        views: { board: "看板", list: "列表", table: "表格", map: "映射" },
        stateTitle: "正在显示最近快照",
        stateDescription: "可以重新连接，当前焦点不会丢失。",
        reconnect: "重新连接",
      }

  return (
    <div className="foundationStory" data-density="comfortable">
      <main className="storySurface" aria-labelledby="environment-title">
        <Card variant="transparent" padding={6}>
          <VStack gap={6}>
          <header className="storyHeader">
            <HStack gap={2} align="center" wrap="wrap">
              <Badge variant="info" label="Storybook" />
              <Text type="supporting" color="secondary">{english ? "Foundations" : "基础层"}</Text>
            </HStack>
            <Heading level={1} id="environment-title">{copy.title}</Heading>
            <Text as="p" type="body" color="secondary">{copy.description}</Text>
          </header>

          <section className="storySection" aria-labelledby="environment-controls-title">
            <Heading level={2} id="environment-controls-title">{english ? "Semantic controls" : "语义控件"}</Heading>
            <HStack gap={4} align="end" wrap="wrap">
              <div className="environmentField">
                <label htmlFor="environment-search">{copy.field}</label>
                <input id="environment-search" type="search" placeholder={copy.placeholder} aria-describedby="environment-search-hint" />
                <small id="environment-search-hint">{copy.hint}</small>
              </div>
              <Button type="button" label={copy.action} variant="primary" />
            </HStack>
            <StorySegmented
              label={english ? "Display view" : "展示视图"}
              value={view}
              onChange={setView}
              options={[
                { value: "board", label: copy.views.board },
                { value: "list", label: copy.views.list },
                { value: "table", label: copy.views.table },
                { value: "map", label: copy.views.map },
              ]}
            />
          </section>

          <section className="storySplit" aria-labelledby="environment-states-title">
            <div className="storySection">
              <Heading level={2} id="environment-states-title">{english ? "State boundary" : "状态边界"}</Heading>
              <article className="environmentState" role="status">
                <div className="environmentStateHeader">
                  <span className="environmentStateMarker" aria-hidden="true" />
                  <Text as="h3" type="label">{copy.stateTitle}</Text>
                </div>
                <Text as="p" type="supporting" color="secondary">{copy.stateDescription}</Text>
                <Button type="button" label={copy.reconnect} variant="secondary" />
              </article>
            </div>
            <div className="storySection">
              <Heading level={2}>{english ? "Focus and status" : "焦点与状态"}</Heading>
              <HStack className="densityRow" align="center" justify="between" gap={3}>
                <Text type="supporting" color="secondary">{copy.status}</Text>
                <Badge variant="success" label={english ? "Ready" : "就绪"} />
              </HStack>
            </div>
          </section>
          </VStack>
        </Card>
      </main>
    </div>
  )
}

export const Baseline: Story = {
  render: (_, context) => <EnvironmentStory english={context.globals.locale === "en"} />,
}
