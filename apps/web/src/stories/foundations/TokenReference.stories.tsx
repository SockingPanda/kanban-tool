import type { Meta, StoryObj } from "@storybook/react-vite"

import { Badge } from "@astryxdesign/core/Badge"
import { Card } from "@astryxdesign/core/Card"
import { HStack } from "@astryxdesign/core/HStack"
import { Heading } from "@astryxdesign/core/Heading"
import { Text } from "@astryxdesign/core/Text"
import { VStack } from "@astryxdesign/core/VStack"

import { typographyTokens } from "../../ui/foundations"

import "./foundations.stories.css"

const meta = {
  title: "Foundations/Token reference",
  parameters: {
    layout: "fullscreen",
    docs: {
      description: {
        component: "App-owned static tokens for color, spacing, radius, focus, density and motion; general controls remain Astryx-owned.",
      },
    },
  },
} satisfies Meta

export default meta
type Story = StoryObj<typeof meta>

const colorTokens = [
  { key: "canvas", variable: "--kb-color-canvas", value: "light #f6f7f8 / dark #0f1113" },
  { key: "surface", variable: "--kb-color-surface", value: "light #ffffff / dark #14171a" },
  { key: "layer", variable: "--kb-color-layer", value: "light #f0f2f4 / dark #191d21" },
  { key: "active", variable: "--kb-color-layer-active", value: "light #e7edf3 / dark #20262c" },
  { key: "border", variable: "--kb-color-border", value: "light #dfe3e7 / dark #2a2f35" },
  { key: "borderStrong", variable: "--kb-color-border-strong", value: "light #c6cdd4 / dark #3a424a" },
  { key: "textPrimary", variable: "--kb-color-text-primary", value: "light #15181c / dark #f4f6f8" },
  { key: "textSecondary", variable: "--kb-color-text-secondary", value: "light #59616a / dark #a7afb8" },
  { key: "textTertiary", variable: "--kb-color-text-tertiary", value: "light #68727c / dark #858f99" },
  { key: "accent", variable: "--kb-color-accent", value: "light #0877bd / dark #0d6fa8" },
  { key: "success", variable: "--kb-color-success", value: "light #147a3d / dark #46c878" },
  { key: "warning", variable: "--kb-color-warning", value: "light #9a6500 / dark #e9ac38" },
  { key: "danger", variable: "--kb-color-danger", value: "light #b42332 / dark #ef6570" },
] as const

const spacingTokens = [
  ["space-1", "--kb-space-1", "4px"],
  ["space-2", "--kb-space-2", "8px"],
  ["space-3", "--kb-space-3", "12px"],
  ["space-4", "--kb-space-4", "16px"],
  ["space-5", "--kb-space-5", "20px"],
  ["space-6", "--kb-space-6", "24px"],
  ["space-8", "--kb-space-8", "32px"],
  ["control-inline", "--kb-space-control-inline", "12px"],
  ["section", "--kb-space-section", "24px"],
] as const

const radiusTokens = [
  ["control", "--kb-radius-control", "6px"],
  ["card", "--kb-radius-card", "8px"],
  ["popover", "--kb-radius-popover", "10px"],
  ["full", "--kb-radius-full", "9999px"],
] as const

const focusTokens = [
  ["ringWidth", "--kb-focus-ring-width", "2px"],
  ["ringOffset", "--kb-focus-ring-offset", "2px"],
  ["ring", "--kb-focus-ring", "canvas gap + accent ring"],
] as const

const focusLabels = {
  zh: { ringWidth: "焦点环宽度", ringOffset: "焦点环间隔", ring: "焦点环" },
  en: { ringWidth: "Ring width", ringOffset: "Ring offset", ring: "Ring" },
} as const

const focusValues = {
  zh: { ringWidth: "2px", ringOffset: "2px", ring: "画布间隔 + 强调色焦点环" },
  en: { ringWidth: "2px", ringOffset: "2px", ring: "canvas gap + accent ring" },
} as const

const copy = {
  zh: {
    eyebrow: "基础层",
    title: "应用自有静态 tokens",
    description: "颜色、间距、圆角、焦点、密度与动效事实集中在这里；通用组件由 Astryx 提供。",
    colors: "颜色",
    spacing: "间距",
    radius: "圆角",
    focus: "焦点",
    typography: "排版",
    density: "密度",
    motion: "动效",
    light: "浅色",
    dark: "深色",
    compact: "紧凑",
    comfortable: "舒适",
    row: "行",
    control: "控件",
    focusSample: "聚焦查看焦点 ring",
    motionText: "交互反馈使用 150ms ease-out；prefers-reduced-motion 会移除非必要动画。",
    densityText: "data-density 同时兼容现有 html[data-density] 与 story-local scope。",
  },
  en: {
    eyebrow: "Foundations",
    title: "App-owned static tokens",
    description: "Color, spacing, radius, focus, density and motion facts live here; general components remain Astryx-owned.",
    colors: "Colors",
    spacing: "Spacing",
    radius: "Radius",
    focus: "Focus",
    typography: "Typography",
    density: "Density",
    motion: "Motion",
    light: "Light",
    dark: "Dark",
    compact: "Compact",
    comfortable: "Comfortable",
    row: "row",
    control: "control",
    focusSample: "Focus to inspect the ring",
    motionText: "Interaction feedback uses 150ms ease-out; prefers-reduced-motion removes non-essential motion.",
    densityText: "data-density works with the existing html[data-density] and story-local scopes.",
  },
} as const

const labels = {
  zh: {
    canvas: "画布",
    surface: "表面",
    layer: "层",
    active: "活动层",
    border: "边框",
    borderStrong: "强调边框",
    textPrimary: "主要文本",
    textSecondary: "次要文本",
    textTertiary: "三级文本",
    accent: "强调色",
    success: "成功",
    warning: "警告",
    danger: "危险",
    body: "正文",
    supporting: "辅助文本",
    code: "代码",
    title: "标题",
  },
  en: {
    canvas: "Canvas",
    surface: "Surface",
    layer: "Layer",
    active: "Active layer",
    border: "Border",
    borderStrong: "Strong border",
    textPrimary: "Primary text",
    textSecondary: "Secondary text",
    textTertiary: "Tertiary text",
    accent: "Accent",
    success: "Success",
    warning: "Warning",
    danger: "Danger",
    body: "Body",
    supporting: "Supporting",
    code: "Code",
    title: "Title",
  },
} as const

function TokenReferenceStory({ english }: { readonly english: boolean }) {
  const language = english ? "en" : "zh"
  const words = copy[language]
  const localizedLabels = labels[language]

  return (
    <div className="foundationStory" data-density="comfortable">
      <main className="storySurface" aria-labelledby="tokens-title">
        <Card variant="transparent" padding={6}>
          <VStack gap={6}>
            <header className="storyHeader">
              <HStack gap={2} align="center" wrap="wrap">
                <Badge variant="info" label={words.eyebrow} />
                <Text type="supporting" color="secondary">{words.motion}</Text>
              </HStack>
              <Heading level={1} id="tokens-title">{words.title}</Heading>
              <Text as="p" type="body" color="secondary">{words.description}</Text>
            </header>

            <section className="storySection" aria-labelledby="token-colors-title">
              <Heading level={2} id="token-colors-title">{words.colors}</Heading>
              <div className="tokenGrid">
                {colorTokens.map((token) => (
                  <div className="tokenItem" key={token.variable}>
                    <span className={`tokenSwatch swatch-${token.key}`} aria-hidden="true" />
                    <Text type="label">{localizedLabels[token.key as keyof typeof localizedLabels]}</Text>
                    <code className="tokenCode">{token.variable} · {token.value}</code>
                  </div>
                ))}
              </div>
            </section>

            <section className="storySplit" aria-label={english ? "Spacing and shape" : "间距与形状"}>
              <div className="storySection">
                <Heading level={2}>{words.spacing}</Heading>
                <div className="tokenList">
                  {spacingTokens.map(([label, variable, value]) => (
                    <div className="spacingItem" key={variable}>
                      <span className={`spacingBar spacing-${label === "control-inline" || label === "section" ? "4" : label.replace("space-", "")}`} aria-hidden="true" />
                      <Text type="label">{label}</Text>
                      <code className="tokenCode">{variable} · {value}</code>
                    </div>
                  ))}
                </div>
              </div>
              <div className="storySection">
                <Heading level={2}>{words.radius}</Heading>
                <div className="tokenList">
                  {radiusTokens.map(([label, variable, value]) => (
                    <div className="tokenRow" key={variable}>
                      <Text type="label">{label}</Text>
                      <code className="tokenCode">{variable} · {value}</code>
                    </div>
                  ))}
                </div>
              </div>
            </section>

            <section className="storySplit" aria-label={english ? "Focus and typography" : "焦点与排版"}>
              <div className="storySection">
                <Heading level={2}>{words.focus}</Heading>
                <div className="tokenList">
                  {focusTokens.map(([key, variable]) => (
                    <div className="tokenRow" key={variable}>
                      <Text type="label">{focusLabels[language][key]}</Text>
                      <code className="tokenCode">{variable} · {focusValues[language][key]}</code>
                    </div>
                  ))}
                </div>
                <button className="focusSample" type="button">{words.focusSample}</button>
              </div>
              <div className="storySection">
                <Heading level={2}>{words.typography}</Heading>
                <div className="typeItem" aria-hidden="true">
                  <Text type="label">{english ? "Token" : "名称"}</Text>
                  <Text type="label">{english ? "Variable" : "变量"}</Text>
                  <Text type="label">{english ? "Use" : "用途"}</Text>
                </div>
                {typographyTokens.map((token) => (
                  <div className="typeItem" key={token.variable}>
                    <Text type={token.key === "title" ? "large" : token.key === "code" ? "code" : "body"}>{localizedLabels[token.key]}</Text>
                    <code className="tokenCode">{token.variable} · {token.value}</code>
                    <Text type="supporting" color="secondary">
                      {token.key === "body" ? (english ? "Daily operation and data reading" : "日常操作与数据阅读")
                        : token.key === "supporting" ? (english ? "Helper copy and dense metadata" : "辅助说明与密集元数据")
                          : token.key === "code" ? (english ? "IDs, refs, run IDs and hashes" : "ID、ref、run id 与 hash")
                            : (english ? "Page or resource titles" : "页面或资源标题")}
                    </Text>
                  </div>
                ))}
              </div>
            </section>

            <section className="storySection" aria-labelledby="token-density-title">
              <Heading level={2} id="token-density-title">{words.density}</Heading>
              <Text as="p" type="supporting" color="secondary">{words.densityText}</Text>
              <div className="densityGrid">
                <div className="densityPanel" data-density="compact">
                  <Text type="label">{words.compact}</Text>
                  <div className="densityRow"><Text type="supporting">40px {words.row}</Text><span className="densityControl">28px {words.control}</span></div>
                </div>
                <div className="densityPanel" data-density="comfortable">
                  <Text type="label">{words.comfortable}</Text>
                  <div className="densityRow"><Text type="supporting">48px {words.row}</Text><span className="densityControl">32px {words.control}</span></div>
                </div>
              </div>
            </section>

            <section className="storySection" aria-labelledby="token-motion-title">
              <Heading level={2} id="token-motion-title">{words.motion}</Heading>
              <Text as="p" type="body" color="secondary">{words.motionText}</Text>
              <code className="tokenCode">--kb-duration-fast · 150ms · --kb-ease-standard · cubic-bezier(0.2, 0, 0, 1)</code>
            </section>
          </VStack>
        </Card>
      </main>
    </div>
  )
}

export const AllTokens: Story = {
  render: (_, context) => <TokenReferenceStory english={context.globals.locale === "en"} />,
}
