import {
  cloneElement,
  isValidElement,
  useEffect,
  useId,
  useRef,
  useState,
  type KeyboardEvent,
  type MouseEvent,
  type ReactElement,
  type ReactNode,
  type RefObject,
} from "react"

import {
  classNames,
  isTopOverlay,
  supportsNativePopover,
  useOverlayInteraction,
} from "./overlay-runtime"

export type PopoverPlacement = "above" | "below" | "start" | "end"
export type PopoverAlignment = "start" | "center" | "end"
export type PopoverSize = "sm" | "md" | "lg"

export const POPOVER_PLACEMENT_CLASSES: Readonly<Record<PopoverPlacement, string>> = Object.freeze({
  above: "bottom-full mb-2",
  below: "top-full mt-2",
  start: "end-full me-2 top-0",
  end: "start-full ms-2 top-0",
})

export const POPOVER_ALIGNMENT_CLASSES: Readonly<Record<PopoverAlignment, string>> = Object.freeze({
  start: "start-0",
  center: "start-1/2 -translate-x-1/2",
  end: "end-0",
})

export const POPOVER_SIZE_CLASSES: Readonly<Record<PopoverSize, string>> = Object.freeze({
  sm: "max-w-xs",
  md: "max-w-sm",
  lg: "max-w-lg",
})

type NativePopoverProps = {
  readonly id?: string
  readonly title?: string
  readonly "aria-label"?: string
  readonly "aria-labelledby"?: string
  readonly "aria-describedby"?: string
}

export interface PopoverTriggerRenderProps {
  readonly ref?: RefObject<HTMLElement | null>
  readonly onClick: () => void
  readonly "aria-haspopup": "dialog" | "true"
  readonly "aria-expanded": boolean
  readonly "aria-controls": string
}

export interface PopoverProps extends NativePopoverProps {
  readonly children?: ReactNode | ((props: PopoverTriggerRenderProps) => ReactNode)
  readonly anchorRef?: RefObject<HTMLElement | null>
  readonly content: ReactNode
  readonly placement?: PopoverPlacement
  readonly alignment?: PopoverAlignment
  readonly size?: PopoverSize
  readonly isOpen?: boolean
  readonly onOpenChange?: (isOpen: boolean) => void
  readonly isEnabled?: boolean
  readonly label?: string
  readonly role?: "dialog" | "none"
  readonly isModal?: boolean
  readonly hasAutoFocus?: boolean
  readonly hasLightDismiss?: boolean
  readonly hasEscapeDismiss?: boolean
  readonly returnFocusRef?: RefObject<HTMLElement | null>
  readonly initialFocusRef?: RefObject<HTMLElement | null>
  readonly className?: string
  readonly "data-testid"?: string
}

function mergeTriggerElement(
  child: ReactElement,
  open: boolean,
  popupId: string,
  onToggle: () => void,
  onKeyDown: (event: KeyboardEvent<HTMLElement>) => void,
  role: "dialog" | "none",
): ReactElement {
  const childProps = child.props as {
    onClick?: (event: MouseEvent<HTMLElement>) => void
    onKeyDown?: (event: KeyboardEvent<HTMLElement>) => void
    "aria-describedby"?: string
  }
  return cloneElement(child as ReactElement<Record<string, unknown>>, {
    "aria-haspopup": role === "dialog" ? "dialog" : "true",
    "aria-expanded": open,
    "aria-controls": popupId,
    onClick: (event: MouseEvent<HTMLElement>) => {
      childProps.onClick?.(event)
      if (!event.defaultPrevented) onToggle()
    },
    onKeyDown: (event: KeyboardEvent<HTMLElement>) => {
      childProps.onKeyDown?.(event)
      if (!event.defaultPrevented) onKeyDown(event)
    },
  })
}

export function Popover({
  children,
  anchorRef,
  content,
  placement = "below",
  alignment = "start",
  size = "md",
  isOpen: controlledOpen,
  onOpenChange,
  isEnabled = true,
  label,
  role = "dialog",
  isModal = false,
  hasAutoFocus = true,
  hasLightDismiss = true,
  hasEscapeDismiss = true,
  returnFocusRef,
  initialFocusRef,
  className,
  id,
  "aria-label": ariaLabel,
  "aria-labelledby": ariaLabelledBy,
  "data-testid": testId,
  ...props
}: PopoverProps) {
  const generatedId = useId()
  const popupId = id ?? generatedId
  const popupRef = useRef<HTMLElement | null>(null)
  const rootRef = useRef<HTMLElement | null>(null)
  const triggerRef = useRef<HTMLElement | null>(null)
  const restoreFocusRef = useRef<HTMLElement | null>(null)
  const [internalOpen, setInternalOpen] = useState(false)
  const isControlled = controlledOpen !== undefined
  const open = isControlled ? controlledOpen : internalOpen
  const openRef = useRef(open)
  const callbackRef = useRef(onOpenChange)
  const lastHideRef = useRef(0)
  openRef.current = open
  callbackRef.current = onOpenChange

  const setOpen = (next: boolean) => {
    if (!isControlled) setInternalOpen(next)
    callbackRef.current?.(next)
  }

  const close = () => {
    if (openRef.current) {
      lastHideRef.current = Date.now()
      setOpen(false)
    }
  }
  const toggle = () => {
    if (!isEnabled || Date.now() - lastHideRef.current < 50) return
    setOpen(!openRef.current)
  }
  const closeRef = useRef(close)
  const toggleRef = useRef(toggle)
  closeRef.current = close
  toggleRef.current = toggle

  useOverlayInteraction(popupRef, {
    enabled: open,
    restoreFocusRef: returnFocusRef ?? restoreFocusRef,
    onEscape: () => {
      if (hasEscapeDismiss && popupRef.current && isTopOverlay(popupRef.current)) closeRef.current()
    },
    trapFocus: isModal,
    autoFocus: hasAutoFocus,
  })

  useEffect(() => {
    if (!open || !hasAutoFocus || !initialFocusRef?.current) return
    const initialFocus = initialFocusRef.current
    const frame = typeof requestAnimationFrame === "function"
      ? requestAnimationFrame(() => {
        if (popupRef.current?.contains(initialFocus)) initialFocus.focus({ preventScroll: true })
      })
      : window.setTimeout(() => {
        if (popupRef.current?.contains(initialFocus)) initialFocus.focus({ preventScroll: true })
      }, 0)
    return () => {
      if (typeof frame === "number") {
        if (typeof cancelAnimationFrame === "function") cancelAnimationFrame(frame)
        else window.clearTimeout(frame)
      }
    }
  }, [hasAutoFocus, initialFocusRef, open])

  useEffect(() => {
    const popup = popupRef.current
    if (!popup) return
    // Anchorless native popovers enter the viewport top layer and cannot honor
    // this component's finite placement map. Detect support for diagnostics,
    // then keep the visible surface DOM-contained for deterministic geometry.
    popup.dataset.popoverSupport = supportsNativePopover(popup) ? "native" : "fallback"
  }, [])

  useEffect(() => {
    if (!open || !hasLightDismiss) return
    const onPointerDown = (event: PointerEvent) => {
      const popup = popupRef.current
      const root = rootRef.current
      const anchor = anchorRef?.current
      if (!popup || !root || !isTopOverlay(popup)) return
      const target = event.target
      if (target instanceof Node && !root.contains(target) && !anchor?.contains(target)) closeRef.current()
    }
    document.addEventListener("pointerdown", onPointerDown, true)
    return () => document.removeEventListener("pointerdown", onPointerDown, true)
  }, [anchorRef, hasLightDismiss, open])

  useEffect(() => {
    const anchor = anchorRef?.current
    if (!anchor) return
    const previous = anchor.getAttribute("aria-controls")
    const previousExpanded = anchor.getAttribute("aria-expanded")
    const previousHasPopup = anchor.getAttribute("aria-haspopup")
    anchor.setAttribute("aria-controls", popupId)
    anchor.setAttribute("aria-expanded", String(open))
    anchor.setAttribute("aria-haspopup", role === "dialog" ? "dialog" : "true")
    const onClick = () => toggleRef.current()
    const onKeyDown = (event: globalThis.KeyboardEvent) => {
      if (event.key === "Enter" || event.key === " ") {
        event.preventDefault()
        toggleRef.current()
      }
    }
    if (isEnabled) {
      anchor.addEventListener("click", onClick)
      anchor.addEventListener("keydown", onKeyDown)
    }
    return () => {
      if (previous) anchor.setAttribute("aria-controls", previous)
      else anchor.removeAttribute("aria-controls")
      if (previousExpanded) anchor.setAttribute("aria-expanded", previousExpanded)
      else anchor.removeAttribute("aria-expanded")
      if (previousHasPopup) anchor.setAttribute("aria-haspopup", previousHasPopup)
      else anchor.removeAttribute("aria-haspopup")
      anchor.removeEventListener("click", onClick)
      anchor.removeEventListener("keydown", onKeyDown)
    }
  }, [anchorRef, isEnabled, open, popupId, role])

  const handleTriggerKeyDown = (event: KeyboardEvent<HTMLElement>) => {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault()
      toggle()
    }
  }

  const triggerProps: PopoverTriggerRenderProps = {
    ref: triggerRef,
    onClick: toggle,
    "aria-haspopup": role === "dialog" ? "dialog" : "true",
    "aria-expanded": open,
    "aria-controls": popupId,
  }

  let trigger: ReactNode = null
  if (!anchorRef && typeof children === "function") {
    trigger = children(triggerProps)
  } else if (!anchorRef && children !== undefined && typeof children !== "function") {
    trigger = isValidElement(children)
      ? mergeTriggerElement(children, open, popupId, toggle, handleTriggerKeyDown, role)
      : children
  }

  return (
    <section ref={rootRef} className="relative inline-flex" aria-label={label ?? ariaLabel}>
      {trigger}
      <aside
        {...props}
        ref={popupRef}
        id={popupId}
        role={role}
        aria-label={label ?? ariaLabel}
        aria-labelledby={ariaLabelledBy}
        aria-modal={role === "dialog" && isModal ? "true" : undefined}
        aria-hidden={!open}
        data-open={open ? "true" : "false"}
        data-testid={testId}
        className={classNames(
          open ? "block" : "hidden",
          "absolute z-40 rounded-lg border border-border bg-popover p-3 text-primary shadow-lg",
          POPOVER_PLACEMENT_CLASSES[placement],
          POPOVER_ALIGNMENT_CLASSES[alignment],
          POPOVER_SIZE_CLASSES[size],
          className,
        )}
      >
        {content}
      </aside>
    </section>
  )
}

Popover.displayName = "AstryxPopover"

export const AstryxPopover = Popover
