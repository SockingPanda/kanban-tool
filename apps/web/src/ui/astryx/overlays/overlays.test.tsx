import { readFileSync } from "node:fs"
import { renderToStaticMarkup } from "react-dom/server"
import { describe, expect, test, vi } from "vitest"

import {
  DIALOG_PLACEMENT_CLASSES,
  DIALOG_SIZE_CLASSES,
  Dialog,
  DropdownMenu,
  MoreMenu,
  POPOVER_ALIGNMENT_CLASSES_BY_PLACEMENT,
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
    expect(POPOVER_ALIGNMENT_CLASSES_BY_PLACEMENT).toEqual({
      above: { start: "start-0", center: "start-1/2 -translate-x-1/2", end: "end-0" },
      below: { start: "start-0", center: "start-1/2 -translate-x-1/2", end: "end-0" },
      start: { start: "top-0", center: "top-1/2 -translate-y-1/2", end: "bottom-0" },
      end: { start: "top-0", center: "top-1/2 -translate-y-1/2", end: "bottom-0" },
    })
    expect(Object.values(DIALOG_SIZE_CLASSES).join(" ")).not.toMatch(/\[[^\]]+\]/)
    expect(Object.values(POPOVER_PLACEMENT_CLASSES).join(" ")).not.toMatch(/\[[^\]]+\]/)
    expect(Object.values(POPOVER_ALIGNMENT_CLASSES_BY_PLACEMENT).flatMap((map) => Object.values(map)).join(" ")).not.toMatch(/\[[^\]]+\]/)
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
    expect(popover).toContain("trapFocus: false")
    expect(runtime).toContain("showPopover()")
    expect(runtime).toContain("hidePopover()")
    expect(popover).toContain("DOM-contained, non-modal static placement")
    expect(popover).not.toMatch(/\bisModal\b/)
    expect(popover).not.toContain("aria-modal")
    for (const file of ["Popover.tsx", "Tooltip.tsx", "DropdownMenu.tsx"]) {
      expect(readFileSync(new URL(`./${file}`, import.meta.url), "utf8")).toContain("relative inline-flex overflow-visible")
    }
    const tooltip = readFileSync(new URL("./Tooltip.tsx", import.meta.url), "utf8")
    expect(tooltip).toContain("bg-surface")
    expect(tooltip).toContain("border border-border")
    expect(tooltip).not.toContain("bg-inverted")
    expect(dialog).toContain('nativeOpen ? "flex" : "hidden"')
    expect(dialog).toContain("setNativeOpen(true)")
    expect(dialog).toContain("setNativeOpen(false)")
    expect(dialog).toContain("callbackRef.current(true)")
  })

  test("rolls Dialog back to its native-open state when close throws", () => {
    const dialog = readFileSync(new URL("./Dialog.tsx", import.meta.url), "utf8")
    expect(dialog).toMatch(
      /dialog\.close\(\)\s*setNativeOpen\(false\)\s*}\s*catch \(error\) \{\s*setNativeOpen\(true\)\s*runtimeErrorRef\.current\?\.\(error\)\s*callbackRef\.current\(true\)/,
    )
  })

  test("requires caller-owned accessible labels instead of English defaults", () => {
    const dropdown = readFileSync(new URL("./DropdownMenu.tsx", import.meta.url), "utf8")
    const moreMenu = readFileSync(new URL("./MoreMenu.tsx", import.meta.url), "utf8")
    expect(dropdown).not.toContain('label: "Menu"')
    expect(dropdown).not.toContain('"Menu"')
    expect(moreMenu).not.toContain('"More options"')

    const markup = renderToStaticMarkup(
      <DropdownMenu
        button={{ isIconOnly: true, "aria-label": "更多操作" }}
        items={[{ label: "运行", onClick: vi.fn() }]}
      />,
    )
    expect(markup).toContain('aria-label="更多操作"')
    expect(markup).not.toContain("Menu")
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
    expect(markup).toContain('class="hidden')
    expect(markup).not.toContain('class="flex')
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

  test("supports alert-dialog semantics that keep backdrop clicks inert", () => {
    const markup = renderToStaticMarkup(
      <Dialog isOpen={false} onOpenChange={vi.fn()} role="alertdialog" closeOnBackdrop={false} aria-label="Confirm">
        <button type="button">Cancel</button>
      </Dialog>,
    )

    expect(markup).toContain('role="alertdialog"')
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
    expect(markup).toContain('class="relative inline-flex overflow-visible"')
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
    expect(markup).toContain("bg-surface")
    expect(markup).toContain("border-border")
    expect(markup).not.toContain("bg-inverted")
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
