import { renderToStaticMarkup } from "react-dom/server"
import { describe, expect, test } from "vitest"

import {
  CODE_BLOCK_HEIGHT_CLASSES,
  CodeBlock,
  GRID_COLUMN_CLASSES,
  Grid,
  guardNoRuntimeStyleProps,
  Skeleton,
  SafeCard,
  type SafeCardProps,
  SafeLayout,
  type SafeLayoutProps,
} from "./index"

describe("CSP-safe Astryx primitives", () => {
  test("Grid maps finite columns and density to literal classes", () => {
    const markup = renderToStaticMarkup(
      <Grid columns="auto-md" density="compact" align="start" data-testid="grid">
        <p>evidence</p>
      </Grid>,
    )

    expect(markup).toContain(`class="grid min-w-0 ${GRID_COLUMN_CLASSES["auto-md"]} gap-2 items-start"`)
    expect(markup).toContain('data-columns="auto-md"')
    expect(markup).toContain('data-density="compact"')
    expect(markup).not.toContain("style=")
    expect(markup).not.toContain("<div")
  })

  test("CodeBlock renders plain native pre/code with finite height and resident live status", () => {
    const markup = renderToStaticMarkup(
      <CodeBlock
        code={'const value = "strict-csp"'}
        language="typescript"
        maxHeight="compact"
        hasCopy
        label="Evidence JSON"
        copyLabel="复制代码"
        copiedLabel="已复制"
      />,
    )

    expect(markup).toContain("<pre")
    expect(markup).toContain('<code data-language="typescript">const value = &quot;strict-csp&quot;</code>')
    expect(markup).toContain(CODE_BLOCK_HEIGHT_CLASSES.compact)
    expect(markup).toContain('aria-live="polite"')
    expect(markup).toContain('data-copy-status')
    expect(markup).not.toContain("style=")
    expect(markup).not.toContain("<style")
    expect(markup).not.toContain("astryx-token-")
    expect(markup).not.toMatch(/(?:slate|gray|neutral)-\d+/)
    expect(markup).toContain('aria-label="复制代码"')
    expect(markup).toContain(">复制代码</button>")
    expect(markup).not.toContain("已复制")
  })

  test("Skeleton exposes only finite geometry and delay variants", () => {
    const markup = renderToStaticMarkup(<Skeleton size="card" index={3} />)

    expect(markup).toContain("h-32 w-full")
    expect(markup).toContain('data-size="card"')
    expect(markup).toContain('data-delay="3"')
    expect(markup).not.toContain("style=")
    expect(markup).not.toMatch(/(?:slate|gray|neutral)-\d+/)
    expect(markup).not.toMatch(/delay-\d+/)
  })

  test("runtime guard strips style and dynamic sizing escape hatches", () => {
    const safe = guardNoRuntimeStyleProps({
      className: "safe",
      style: { color: "red" },
      xstyle: {},
      width: "100%",
      height: 240,
      contentWidth: 960,
    })

    expect(safe).toEqual({ className: "safe" })
  })

  test("safe facade types reject runtime styling and dimensions", () => {
    const safeCard: SafeCardProps = { children: "safe", variant: "default" }
    const safeLayout: SafeLayoutProps = { children: "safe" }
    expect(safeCard.children).toBe("safe")
    expect(safeLayout.children).toBe("safe")

    // @ts-expect-error Static facade must not expose dynamic width.
    const width = <SafeCard width={320} />
    // @ts-expect-error Static facade must not expose inline style.
    const style = <SafeCard style={{ color: "red" }} />
    // @ts-expect-error Layout contentWidth is a runtime style escape hatch.
    const contentWidth = <SafeLayout contentWidth={960} />
    expect(width).toBeDefined()
    expect(style).toBeDefined()
    expect(contentWidth).toBeDefined()
  })

  test("safe facade strips runtime escape hatches from spread objects", () => {
    const markup = renderToStaticMarkup(
      <SafeCard
        {...({
          children: "safe",
          width: 320,
          style: { color: "red" },
        } as unknown as SafeCardProps)}
      />,
    )

    expect(markup).toContain("safe")
    expect(markup).not.toContain("style=")
  })
})
