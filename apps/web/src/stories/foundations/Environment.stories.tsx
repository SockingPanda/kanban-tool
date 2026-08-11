import type { Meta, StoryObj } from "@storybook/react-vite"

const meta = {
  title: "Foundations/Environment",
  parameters: {
    layout: "centered",
  },
} satisfies Meta

export default meta
type Story = StoryObj<typeof meta>

export const Baseline: Story = {
  render: () => (
    <main
      aria-labelledby="environment-title"
      style={{
        display: "grid",
        gap: "1.25rem",
        maxWidth: "36rem",
        padding: "2rem",
        color: "var(--color-text-primary)",
        background: "var(--color-background-surface)",
        border: "1px solid var(--color-border)",
        borderRadius: "var(--radius-container)",
      }}
    >
      <header>
        <p style={{ margin: 0, color: "var(--color-text-secondary)" }}>KANBAN TOOL / STORYBOOK</p>
        <h1 id="environment-title">Foundation environment</h1>
        <p style={{ marginBottom: 0 }}>
          A deterministic surface for inspecting Astryx typography, theme modes, semantic markup, and focus states.
        </p>
      </header>

      <section aria-labelledby="semantic-surfaces-title">
        <h2 id="semantic-surfaces-title">Semantic surfaces</h2>
        <div style={{ display: "grid", gap: "0.75rem" }}>
          <label htmlFor="environment-input">Focusable input</label>
          <input id="environment-input" name="environment-input" placeholder="Focus to inspect the ring" />
          <button type="button">Keyboard action</button>
          <a href="#semantic-surfaces-title">Semantic link</a>
        </div>
      </section>

      <p role="status" aria-live="polite" style={{ margin: 0, color: "var(--color-text-secondary)" }}>
        Local-only story: no API, SSE, or mutation path is connected.
      </p>
    </main>
  ),
}
