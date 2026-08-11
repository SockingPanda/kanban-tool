import type { Meta, StoryObj } from "@storybook/react-vite"

import { FoundationFrame, typographyTokens } from "../../ui/foundations"
import { Badge, Inline, Stack, Surface, Text } from "../../ui/primitives"

import "./foundations.stories.css"

const meta = {
  title: "Foundations/Token reference",
  parameters: {
    layout: "fullscreen",
    docs: {
      description: {
        component: "kanban-tool app-owned token reference。颜色、间距、圆角、焦点与密度均为静态 CSS custom properties。",
      },
    },
  },
} satisfies Meta

export default meta
type Story = StoryObj<typeof meta>

const colorTokens = [
  ["Canvas", "--kb-color-canvas", "canvas"],
  ["Surface", "--kb-color-surface", "surface"],
  ["Layer", "--kb-color-layer", "layer"],
  ["Active layer", "--kb-color-layer-active", "active"],
  ["Accent", "--kb-color-accent", "accent"],
  ["Success", "--kb-color-success", "success"],
  ["Warning", "--kb-color-warning", "warning"],
  ["Danger", "--kb-color-danger", "danger"],
] as const

const spacingTokens = [
  ["space-1", "--kb-space-1"],
  ["space-2", "--kb-space-2"],
  ["space-3", "--kb-space-3"],
  ["space-4", "--kb-space-4"],
  ["space-5", "--kb-space-5"],
  ["space-6", "--kb-space-6"],
] as const

export const AllTokens: Story = {
  render: () => (
    <FoundationFrame className="foundationStory">
      <Surface as="main" tone="surface" padding="comfortable" aria-labelledby="tokens-title">
        <Stack gap="loose">
          <header className="storyHeader">
            <Inline gap="tight"><Badge tone="accent">Foundations</Badge><Text as="span" size="supporting" tone="secondary">Tokens</Text></Inline>
            <Text as="h1" id="tokens-title" size="pageTitle">App-owned foundations</Text>
            <Text tone="secondary">静态 token 负责层级、密度与可读性；组件不依赖 runtime inline style。</Text>
          </header>

          <section className="tokenSection" aria-labelledby="token-colors-title">
            <Text as="h2" id="token-colors-title" size="title">Colors / 颜色</Text>
            <div className="colorGrid">
              {colorTokens.map(([label, variable, tone]) => <div className="colorToken" key={variable}><span className={`colorSwatch swatch-${tone}`} aria-hidden="true" /><Text as="span" size="supporting">{label}</Text><Text as="code" size="code" tone="tertiary">{variable}</Text></div>)}
            </div>
          </section>

          <section className="tokenSection" aria-labelledby="token-spacing-title">
            <Text as="h2" id="token-spacing-title" size="title">Spacing / 间距</Text>
            <div className="spacingGrid">
              {spacingTokens.map(([label, variable]) => <div className="spacingToken" key={variable}><span className={`spacingBar spacing-${label.replace("space-", "")}`} aria-hidden="true" /><Text as="code" size="code">{variable}</Text><Text as="span" size="supporting" tone="secondary">{label}</Text></div>)}
            </div>
          </section>

          <section className="tokenSection" aria-labelledby="token-type-title">
            <Text as="h2" id="token-type-title" size="title">Typography / 排版</Text>
            <div className="typeGrid">{typographyTokens.map((token) => <div className="typeToken" key={token.variable}><Text as="span" size={token.name === "Title" ? "title" : token.name === "Code" ? "code" : "body"}>{token.name}</Text><Text as="code" size="code" tone="tertiary">{token.variable}</Text><Text as="span" size="supporting" tone="secondary">{token.detail}</Text></div>)}</div>
          </section>

          <section className="tokenSection" aria-labelledby="token-density-title">
            <Text as="h2" id="token-density-title" size="title">Density / 密度</Text>
            <Inline gap="default">
              <Surface tone="layer" padding="compact" className="densityDemo" data-kb-density="compact"><Text as="strong" size="supporting">Compact</Text><Text as="span" size="supporting" tone="secondary">40px row / 28px control</Text></Surface>
              <Surface tone="layer" padding="compact" className="densityDemo" data-kb-density="comfortable"><Text as="strong" size="supporting">Comfortable</Text><Text as="span" size="supporting" tone="secondary">48px row / 32px control</Text></Surface>
            </Inline>
          </section>
        </Stack>
      </Surface>
    </FoundationFrame>
  ),
}
