import { useEffect, useRef, type ReactElement } from "react"

type OverlayElement = HTMLElement

const overlayStack: OverlayElement[] = []

function removeOverlay(node: OverlayElement): void {
  const index = overlayStack.lastIndexOf(node)
  if (index >= 0) overlayStack.splice(index, 1)
}

export function registerOverlay(node: OverlayElement): () => void {
  removeOverlay(node)
  overlayStack.push(node)
  return () => removeOverlay(node)
}

export function isTopOverlay(node: OverlayElement): boolean {
  return overlayStack[overlayStack.length - 1] === node
}

function isFocusable(node: HTMLElement): boolean {
  if (node.hidden || node.getAttribute("aria-hidden") === "true") return false
  if (node.hasAttribute("disabled")) return false
  if (node.closest("[inert]")) return false
  const tabIndex = node.getAttribute("tabindex")
  if (tabIndex !== null && Number(tabIndex) < 0) return false
  return node.matches(
    "a[href], area[href], button, input, select, textarea, summary, [contenteditable=\"true\"], [tabindex]",
  )
}

export function getFocusableElements(container: HTMLElement): HTMLElement[] {
  return Array.from(
    container.querySelectorAll<HTMLElement>(
      "a[href], area[href], button, input, select, textarea, summary, [contenteditable=\"true\"], [tabindex]",
    ),
  ).filter(isFocusable)
}

function focusFirstOrLast(container: HTMLElement, last: boolean): void {
  const focusable = getFocusableElements(container)
  const target = last ? focusable[focusable.length - 1] : focusable[0]
  if (target) {
    target.focus({ preventScroll: true })
  } else {
    container.focus({ preventScroll: true })
  }
}

export interface OverlayInteractionOptions {
  readonly enabled: boolean
  readonly restoreFocusRef?: { current: HTMLElement | null }
  readonly shouldRestoreFocus?: () => boolean
  readonly onEscape?: () => void
  readonly trapFocus?: boolean
  readonly autoFocus?: boolean
}

/** Shared keyboard and focus behavior for modal and popup overlays. */
export function useOverlayInteraction(
  nodeRef: { current: OverlayElement | null },
  { enabled, restoreFocusRef, shouldRestoreFocus, onEscape, trapFocus = true, autoFocus = true }: OverlayInteractionOptions,
): void {
  const escapeRef = useRef(onEscape)
  const shouldRestoreFocusRef = useRef(shouldRestoreFocus)
  escapeRef.current = onEscape
  shouldRestoreFocusRef.current = shouldRestoreFocus
  const restoreRef = useRef<HTMLElement | null>(null)

  useEffect(() => {
    if (!enabled || typeof document === "undefined") return
    const node = nodeRef.current
    if (!node) return

    restoreRef.current = restoreFocusRef?.current ??
      (document.activeElement instanceof HTMLElement ? document.activeElement : null)
    const unregister = registerOverlay(node)

    const focusInitial = () => {
      if (!node.isConnected || !isTopOverlay(node)) return
      if (autoFocus) focusFirstOrLast(node, false)
    }
    const frame = typeof requestAnimationFrame === "function"
      ? requestAnimationFrame(focusInitial)
      : window.setTimeout(focusInitial, 0)

    const onKeyDown = (event: KeyboardEvent) => {
      if (!isTopOverlay(node)) return

      if (event.key === "Escape") {
        event.preventDefault()
        event.stopPropagation()
        escapeRef.current?.()
        return
      }

      if (!trapFocus || event.key !== "Tab") return
      const focusable = getFocusableElements(node)
      if (focusable.length === 0) {
        event.preventDefault()
        node.focus({ preventScroll: true })
        return
      }

      const active = document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null
      const activeIndex = active ? focusable.indexOf(active) : -1
      if (event.shiftKey) {
        if (activeIndex <= 0) {
          event.preventDefault()
          focusable[focusable.length - 1]?.focus({ preventScroll: true })
        }
      } else if (activeIndex < 0 || activeIndex === focusable.length - 1) {
        event.preventDefault()
        focusable[activeIndex < 0 ? 0 : 0]?.focus({ preventScroll: true })
      }
    }

    const onFocusIn = (event: FocusEvent) => {
      if (!trapFocus || !isTopOverlay(node)) return
      const target = event.target
      if (!(target instanceof Node) || node.contains(target)) return
      focusFirstOrLast(node, false)
    }

    document.addEventListener("keydown", onKeyDown, true)
    document.addEventListener("focusin", onFocusIn, true)

    return () => {
      document.removeEventListener("keydown", onKeyDown, true)
      document.removeEventListener("focusin", onFocusIn, true)
      if (typeof frame === "number") {
        if (typeof cancelAnimationFrame === "function") cancelAnimationFrame(frame)
        else window.clearTimeout(frame)
      }
      unregister()
      const restore = restoreRef.current
      restoreRef.current = null
      if (restore?.isConnected && shouldRestoreFocusRef.current?.() !== false) restore.focus({ preventScroll: true })
    }
  }, [autoFocus, enabled, nodeRef, restoreFocusRef, trapFocus])
}

export function supportsNativePopover(node: HTMLElement | null): boolean {
  return Boolean(
    node &&
    typeof (node as HTMLElement & { showPopover?: unknown }).showPopover === "function" &&
    typeof (node as HTMLElement & { hidePopover?: unknown }).hidePopover === "function",
  )
}

export function openNativePopover(node: HTMLElement): boolean {
  if (!supportsNativePopover(node)) return false
  const popover = node as HTMLElement & { showPopover: () => void }
  popover.showPopover()
  return true
}

export function closeNativePopover(node: HTMLElement): boolean {
  if (!supportsNativePopover(node)) return false
  const popover = node as HTMLElement & { hidePopover: () => void }
  popover.hidePopover()
  return true
}

export function classNames(...values: Array<string | false | null | undefined>): string {
  return values.filter(Boolean).join(" ")
}

export function mergeDescribedBy(...ids: Array<string | null | undefined>): string | undefined {
  const values = ids
    .flatMap((value) => value?.split(/\s+/) ?? [])
    .filter((value, index, all) => value.length > 0 && all.indexOf(value) === index)
  return values.length > 0 ? values.join(" ") : undefined
}

export function isElementNode(value: unknown): value is ReactElement {
  return Boolean(value && typeof value === "object" && "type" in value && "props" in value)
}
