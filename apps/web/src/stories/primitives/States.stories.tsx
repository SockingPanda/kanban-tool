import type { Meta, StoryObj } from "@storybook/react-vite"

import { Button } from "@astryxdesign/core/Button"
import { Card } from "@astryxdesign/core/Card"
import { Heading } from "@astryxdesign/core/Heading"
import { Text } from "@astryxdesign/core/Text"
import { VStack } from "@astryxdesign/core/VStack"

import "./primitives.stories.css"

const meta = {
  title: "Astryx/Resilient states",
  parameters: {
    layout: "fullscreen",
    docs: {
      description: {
        component: "loading、empty、offline、stale、recovering、error 和 retry 是正式可观察状态；组合只属于此 story，不导出通用 primitive。",
      },
    },
  },
} satisfies Meta

export default meta
type Story = StoryObj<typeof meta>

type StateMode = "loading" | "empty" | "offline" | "stale" | "recovering" | "error" | "retry"

function StateCard({
  mode,
  title,
  description,
  action,
  actionVariant = "secondary",
  loading = false,
  english,
}: {
  readonly mode: StateMode
  readonly title: string
  readonly description: string
  readonly action?: string
  readonly actionVariant?: "primary" | "secondary" | "destructive"
  readonly loading?: boolean
  readonly english: boolean
}) {
  const stateLabel = english ? mode : ({
    loading: "加载中",
    empty: "空态",
    offline: "离线",
    stale: "过期",
    recovering: "恢复中",
    error: "错误",
    retry: "重试",
  } satisfies Record<StateMode, string>)[mode]

  return (
    <div className="stateCard" data-state={mode} role={mode === "error" ? "alert" : "status"} aria-live={mode === "error" ? "assertive" : "polite"} aria-label={stateLabel}>
      <span className="stateMarker" aria-hidden="true" />
      <div className="stateContent">
        <Text as="h2" type="label">{title}</Text>
        <Text as="p" type="supporting" color="secondary">{description}</Text>
        {action ? (
          <div className="stateAction">
            <Button
              type="button"
              label={action}
              variant={actionVariant}
              isDisabled={loading}
              aria-busy={loading || undefined}
              icon={loading ? <span className="controlSpinner" aria-hidden="true" /> : undefined}
            />
          </div>
        ) : null}
      </div>
    </div>
  )
}

function StatesStory({ english }: { readonly english: boolean }) {
  const copy = english
    ? {
        title: "Resilient states",
        description: "Every state keeps an explicit message and a recovery affordance where one is safe.",
        loadingTitle: "Loading canonical snapshot",
        loadingDescription: "The surface remains reserved while data arrives.",
        emptyTitle: "No projects yet",
        emptyDescription: "Create a project through the canonical service path.",
        emptyAction: "View guide",
        offlineTitle: "Service is offline",
        offlineDescription: "The last known facts remain available locally.",
        reconnect: "Reconnect",
        staleTitle: "Snapshot may be stale",
        staleDescription: "Reconnect to check for newer events.",
        refresh: "Refresh",
        recoveringTitle: "Recovering the event boundary",
        recoveringDescription: "The latest snapshot is being checked before live updates resume.",
        recovering: "Recovering",
        errorTitle: "Snapshot could not be read",
        errorDescription: "No mutation was attempted. Retry the read.",
        retryRead: "Retry read",
        retryTitle: "Retry is available",
        retryDescription: "The previous read was not accepted; try the canonical read again.",
        retry: "Retry now",
      }
    : {
        title: "恢复状态",
        description: "每个状态都有明确文案；在安全时提供恢复动作。",
        loadingTitle: "正在加载 canonical 快照",
        loadingDescription: "数据到达前保留表面空间。",
        emptyTitle: "还没有项目",
        emptyDescription: "请通过 canonical service path 创建项目。",
        emptyAction: "查看指南",
        offlineTitle: "服务离线",
        offlineDescription: "最近已知事实仍在本机保留。",
        reconnect: "重新连接",
        staleTitle: "快照可能已过期",
        staleDescription: "重新连接以检查更新事件。",
        refresh: "刷新",
        recoveringTitle: "正在恢复事件边界",
        recoveringDescription: "恢复 live 更新前先检查最新快照。",
        recovering: "恢复中",
        errorTitle: "无法读取快照",
        errorDescription: "未尝试任何 mutation；请重试读取。",
        retryRead: "重试读取",
        retryTitle: "可以重试",
        retryDescription: "上一次读取未被接受；请再次执行 canonical read。",
        retry: "立即重试",
      }

  return (
    <div className="primitiveStory">
      <main className="primitiveSurface" aria-labelledby="states-title">
        <Card variant="transparent" padding={6}>
          <VStack gap={6}>
            <header className="storyHeader">
              <Heading level={1} id="states-title">{copy.title}</Heading>
              <Text as="p" type="body" color="secondary">{copy.description}</Text>
            </header>
            <div className="stateGrid">
              <StateCard mode="loading" title={copy.loadingTitle} description={copy.loadingDescription} english={english} />
              <StateCard mode="empty" title={copy.emptyTitle} description={copy.emptyDescription} action={copy.emptyAction} actionVariant="primary" english={english} />
              <StateCard mode="offline" title={copy.offlineTitle} description={copy.offlineDescription} action={copy.reconnect} english={english} />
              <StateCard mode="stale" title={copy.staleTitle} description={copy.staleDescription} action={copy.refresh} english={english} />
              <StateCard mode="recovering" title={copy.recoveringTitle} description={copy.recoveringDescription} action={copy.recovering} loading english={english} />
              <StateCard mode="error" title={copy.errorTitle} description={copy.errorDescription} action={copy.retryRead} actionVariant="destructive" english={english} />
              <StateCard mode="retry" title={copy.retryTitle} description={copy.retryDescription} action={copy.retry} actionVariant="primary" english={english} />
            </div>
          </VStack>
        </Card>
      </main>
    </div>
  )
}

export const ResilientStates: Story = {
  render: (_, context) => <StatesStory english={context.globals.locale === "en"} />,
}
