import { renderToStaticMarkup } from "react-dom/server"
import { describe, expect, test, vi } from "vitest"

import {
  CODE_BLOCK_HEIGHT_CLASSES,
  CODE_BLOCK_COPY_FEEDBACK_MS,
  CodeBlock,
  GRID_COLUMN_CLASSES,
  Grid,
  guardNoRuntimeStyleProps,
  Skeleton,
  SafeCard,
  type SafeCardProps,
  SafeLayout,
  type SafeLayoutProps,
  SafeLayoutPanel,
  SafeMetadataList,
  scheduleCopyFeedbackReset,
  type SafeMetadataListProps,
} from "./index"

describe("CSP-safe Astryx primitives", () => {
  test("Grid maps finite columns and density to literal classes", () => {
    const markup = renderToStaticMarkup(
      <Grid
        label="Evidence grid"
        columns="auto-md"
        density="compact"
        align="start"
        data-testid="grid"
      >
        <p>evidence</p>
      </Grid>,
    )

    expect(markup).toContain(`class="grid min-w-0 ${GRID_COLUMN_CLASSES["auto-md"]} gap-2 items-start"`)
    expect(markup).toContain('aria-label="Evidence grid"')
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
        errorLabel="复制失败"
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

  test("Skeleton exposes finite geometry and a reduced-motion-safe animation", () => {
    const markup = renderToStaticMarkup(<Skeleton size="card" />)

    expect(markup).toContain("motion-safe:animate-pulse")
    expect(markup).toContain("h-32 w-full")
    expect(markup).toContain('data-size="card"')
    expect(markup).not.toContain("data-delay")
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
      resizable: {},
      columns: 3,
    })

    expect(safe).toEqual({ className: "safe" })
  })

  test("safe facade types reject runtime styling and dimensions", () => {
    const safeCard: SafeCardProps = { children: "safe", variant: "default" }
    const safeLayout: SafeLayoutProps = { children: "safe" }
    const safeMetadata: SafeMetadataListProps = {
      children: "safe",
      columns: "multi",
      label: { position: "top" },
    }
    expect(safeCard.children).toBe("safe")
    expect(safeLayout.children).toBe("safe")
    expect(safeMetadata.children).toBe("safe")

    // @ts-expect-error A section landmark must have an accessible label.
    const unlabeledGrid = <Grid><p>safe</p></Grid>
    // @ts-expect-error CodeBlock requires a caller-provided accessible label and copy feedback labels.
    const unlabeledCodeBlock = <CodeBlock code="safe" />
    // @ts-expect-error Static facade must not expose dynamic width.
    const width = <SafeCard width={320} />
    // @ts-expect-error Static facade must not expose inline style.
    const style = <SafeCard style={{ color: "red" }} />
    // @ts-expect-error Layout contentWidth is a runtime style escape hatch.
    const contentWidth = <SafeLayout contentWidth={960} />
    // @ts-expect-error Resizable props drive runtime panel width.
    const resizable = <SafeLayoutPanel resizable={null} />
    // @ts-expect-error MetadataList label width drives a runtime grid track.
    const labelWidth = <SafeMetadataList label={{position: "start", width: 160}}>safe</SafeMetadataList>
    // @ts-expect-error Numeric MetadataList columns drive a runtime grid track.
    const numericColumns = <SafeMetadataList columns={3}>safe</SafeMetadataList>
    expect(width).toBeDefined()
    expect(style).toBeDefined()
    expect(contentWidth).toBeDefined()
    expect(unlabeledGrid).toBeDefined()
    expect(unlabeledCodeBlock).toBeDefined()
    expect(resizable).toBeDefined()
    expect(labelWidth).toBeDefined()
    expect(numericColumns).toBeDefined()
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

  test("safe MetadataList facade strips nested label width from spread objects", () => {
    const markup = renderToStaticMarkup(
      <SafeMetadataList
        {...({
          children: "safe",
          columns: 3,
          label: { position: "start", width: "12rem" },
        } as unknown as SafeMetadataListProps)}
      />,
    )

    expect(markup).toContain("safe")
    expect(markup).not.toContain("12rem")
    expect(markup).not.toContain("repeat(3")
    expect(markup).not.toContain("style=")
  })

  test("copy success reset uses a finite timer", () => {
    vi.useFakeTimers()
    try {
      const setStatus = vi.fn()
      const cancel = scheduleCopyFeedbackReset(setStatus)

      vi.advanceTimersByTime(CODE_BLOCK_COPY_FEEDBACK_MS - 1)
      expect(setStatus).not.toHaveBeenCalled()
      vi.advanceTimersByTime(1)
      expect(setStatus).toHaveBeenCalledWith("idle")

      cancel()
    } finally {
      vi.useRealTimers()
    }
  })
})
