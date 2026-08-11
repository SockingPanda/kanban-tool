import type { Meta, StoryObj } from "@storybook/react-vite"

import { FoundationFrame } from "../../ui/foundations"
import { Inline, Stack, StateBoundary, Surface, Text } from "../../ui/primitives"

import "./primitives.stories.css"

const meta = {
  title: "Primitives/State boundaries",
  parameters: {
    layout: "fullscreen",
    docs: {
      description: {
        component: "loading、empty、offline、stale、error 是正式可观察状态；这里的内容是静态 Storybook fixture。",
      },
    },
  },
} satisfies Meta

export default meta
type Story = StoryObj<typeof meta>

export const ResilientStates: Story = {
  render: (_, context) => {
    const english = context.globals.locale === "en"
    return (
      <FoundationFrame className="primitiveStory">
        <Surface as="main" tone="surface" padding="comfortable" aria-labelledby="states-title">
          <Stack gap="loose">
            <header className="storyHeader">
              <Text as="h1" id="states-title" size="pageTitle">{english ? "Resilient states" : "恢复状态"}</Text>
              <Text tone="secondary">{english ? "Every state keeps an explicit message and recovery affordance." : "每个状态都有明确文案与恢复动作。"}</Text>
            </header>
            <div className="stateGrid">
              <StateBoundary mode="loading" title={english ? "Loading canonical snapshot" : "正在加载 canonical 快照"} description={english ? "The surface remains reserved while data arrives." : "数据到达前保留表面空间。"} />
              <StateBoundary mode="empty" title={english ? "No projects yet" : "还没有项目"} description={english ? "Create a project through the canonical service path." : "请通过 canonical service path 创建项目。"} action={<Inline gap="tight"><button className="storyLink" type="button">{english ? "View guide" : "查看指南"}</button></Inline>} />
              <StateBoundary mode="offline" title={english ? "Service is offline" : "服务离线"} description={english ? "The last known facts remain available locally." : "最近已知事实仍在本机保留。"} onRetry={() => undefined} retryLabel={english ? "Reconnect" : "重新连接"} />
              <StateBoundary mode="stale" title={english ? "Snapshot may be stale" : "快照可能已过期"} description={english ? "Reconnect to check for newer events." : "重新连接以检查更新事件。"} onRetry={() => undefined} retryLabel={english ? "Refresh" : "刷新"} />
              <StateBoundary mode="error" title={english ? "Snapshot could not be read" : "无法读取快照"} description={english ? "No mutation was attempted. Retry the read." : "未尝试任何 mutation；请重试读取。"} onRetry={() => undefined} retryLabel={english ? "Retry read" : "重试读取"} />
            </div>
          </Stack>
        </Surface>
      </FoundationFrame>
    )
  },
}
