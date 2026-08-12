import { readFileSync } from "node:fs"
import { renderToStaticMarkup } from "react-dom/server"
import { describe, expect, test, vi } from "vitest"

import {
  DIALOG_PLACEMENT_CLASSES,
  DIALOG_SIZE_CLASSES,
  Dialog,
  DropdownMenu,
  MoreMenu,
  POPOVER_PLACEMENT_CLASSES,
  Popover,
  Tooltip,
  mergeDescribedBy,
} from "./index"

describe("Astryx CSP-safe overlays", () => {
  test("keeps finite size and placement maps literal and bounded", () => {
    expect(Object.keys(DIALOG_SIZE_CLASSES)).toEqual(["sm", "md", "lg", "xl", "full"])
    expect(Object.keys(DIALOG_PLACEMENT_CLASSES)).toEqual(["center", "top", "bottom"])
    expect(Object.keys(POPOVER_PLACEMENT_CLASSES)).toEqual(["above", "below", "start", "end"])
    expect(Object.values(DIALOG_SIZE_CLASSES).join(" ")).not.toMatch(/\[[^\]]+\]/)
    expect(Object.values(POPOVER_PLACEMENT_CLASSES).join(" ")).not.toMatch(/\[[^\]]+\]/)
  })

  test("keeps the source free of raw layout wrappers, runtime styles, and palette forks", () => {
    const sourceFiles = ["Dialog.tsx", "Popover.tsx", "Tooltip.tsx", "DropdownMenu.tsx", "MoreMenu.tsx", "overlay-runtime.ts"]
    const source = sourceFiles.map((file) => readFileSync(new URL(`./${file}`, import.meta.url), "utf8")).join("\n")
    expect(source).not.toMatch(/<(?:div|span)\b/)
    expect(source).not.toMatch(/\b(?:style|xstyle|anchorName|positionArea)\s*[:=]/)
    expect(source).not.toMatch(/(?:bg|text|border)-(?:slate|sky|white|black)\b/)
    expect(source).not.toMatch(/(?:bg-accent-strong|text-on-inverted|bg-inverted-strong)/)
    expect(source).toMatch(/(?:bg-surface|bg-popover|bg-muted|bg-inverted|bg-accent-bg|text-primary|text-secondary|text-on-accent|border-border|border-border-strong|outline-accent)/)
    expect(source).not.toMatch(/dark:/)
    expect(source.split(/\s+/).some((token) => token.includes("-["))).toBe(false)
  })

  test("keeps native focus and keyboard paths explicit", () => {
    const dialog = readFileSync(new URL("./Dialog.tsx", import.meta.url), "utf8")
    const runtime = readFileSync(new URL("./overlay-runtime.ts", import.meta.url), "utf8")
    const popover = readFileSync(new URL("./Popover.tsx", import.meta.url), "utf8")
    expect(dialog).toContain("dialog.showModal()")
    expect(dialog).toContain("dialog.close()")
    expect(runtime).toContain('event.key === "Escape"')
    expect(runtime).toContain('event.key !== "Tab"')
    expect(runtime).toContain('document.addEventListener("focusin"')
    expect(runtime).toContain("restoreFocusRef")
    expect(runtime).toContain("registerOverlay(node)")
    expect(runtime).toContain("shouldRestoreFocus")
    expect(popover).toContain("supportsNativePopover")
    expect(popover).toContain("trapFocus: isModal")
    expect(runtime).toContain("showPopover()")
    expect(runtime).toContain("hidePopover()")
  })

  test("renders a named native dialog on the server without inline styling", () => {
    const markup = renderToStaticMarkup(
      <Dialog isOpen onOpenChange={vi.fn()} aria-label="Confirm" data-testid="confirm-dialog">
        <button type="button">Confirm</button>
      </Dialog>,
    )

    expect(markup).toContain("<dialog")
    expect(markup).toContain('aria-label="Confirm"')
    expect(markup).toContain('aria-modal="true"')
    expect(markup).toContain('data-testid="confirm-dialog"')
    expect(markup).not.toContain(" style=")
    expect(markup).not.toContain("<style")
  })

  test("keeps a closed dialog hydration-safe on the server", () => {
    const markup = renderToStaticMarkup(
      <Dialog isOpen={false} onOpenChange={vi.fn()} aria-label="Confirm" data-testid="closed-dialog">
        <button type="button">Confirm</button>
      </Dialog>,
    )

    expect(markup).toContain('data-open="false"')
    expect(markup).toContain('data-testid="closed-dialog"')
    expect(markup).not.toContain(" open")
    expect(markup).not.toContain(" style=")
  })

  test("keeps a controlled popover DOM-contained with static placement classes", () => {
    const markup = renderToStaticMarkup(
      <Popover
        isOpen
        onOpenChange={vi.fn()}
        label="Filters"
        placement="below"
        data-testid="filters-popover"
        content={<button type="button">Apply</button>}
      >
        <button type="button">Open filters</button>
      </Popover>,
    )

    expect(markup).toContain('aria-expanded="true"')
    expect(markup).toContain('aria-haspopup="dialog"')
    expect(markup).toContain('role="dialog"')
    expect(markup).not.toContain('aria-modal="true"')
    expect(markup).toContain('class="relative inline-flex"')
    expect(markup).toContain("top-full")
    expect(markup).toContain('data-testid="filters-popover"')
    expect(markup).not.toContain(" style=")
  })

  test("publishes static describedby tooltip content and preserves existing ids", () => {
    const markup = renderToStaticMarkup(
      <Tooltip content="More detail" id="detail-tip">
        <button type="button" aria-describedby="existing-help">Details</button>
      </Tooltip>,
    )

    expect(markup).toContain('aria-describedby="existing-help detail-tip"')
    expect(markup).toContain('id="detail-tip"')
    expect(markup).toContain('role="tooltip"')
    expect(markup).toContain("More detail")
    expect(markup).toContain("sr-only")
    expect(markup).not.toContain('aria-hidden="true"')
    expect(markup).not.toContain('class="hidden"')
    expect(markup).not.toContain(" style=")
  })

  test("renders menu semantics and MoreMenu's accessible label on the server", () => {
    const markup = renderToStaticMarkup(
      <MoreMenu
        label="Task actions"
        items={[
          { label: "Edit", onClick: vi.fn() },
          { type: "divider" },
          { label: "Delete", isDisabled: true },
        ]}
        data-testid="task-more-menu"
      />,
    )

    expect(markup).toContain('aria-label="Task actions"')
    expect(markup).toContain('aria-haspopup="menu"')
    expect(markup).toContain('role="menu"')
    expect(markup).toContain('role="menuitem"')
    expect(markup).toContain('data-testid="task-more-menu"')
    expect(markup).not.toContain(" style=")
  })

  test("keeps controlled menu state in the rendered contract", () => {
    const onOpenChange = vi.fn()
    const markup = renderToStaticMarkup(
      <DropdownMenu
        isMenuOpen
        onOpenChange={onOpenChange}
        button={{ label: "Actions" }}
        items={[{ label: "Run", onClick: vi.fn() }]}
      />,
    )

    expect(markup).toContain('aria-expanded="true"')
    expect(markup).toContain('data-open="true"')
    expect(markup).not.toContain(" style=")
  })

  test("merges describedby ids deterministically", () => {
    expect(mergeDescribedBy("first second", "second third", undefined)).toBe("first second third")
    expect(mergeDescribedBy(undefined, null)).toBeUndefined()
  })
})
