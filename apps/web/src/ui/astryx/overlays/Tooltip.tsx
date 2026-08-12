import {
  cloneElement,
  useCallback,
  isValidElement,
  useEffect,
  useId,
  useRef,
  useState,
  type FocusEvent,
  type KeyboardEvent,
  type MouseEvent,
  type ReactElement,
  type ReactNode,
  type RefObject,
} from "react"

import { classNames, mergeDescribedBy } from "./overlay-runtime"
import type { PopoverAlignment, PopoverPlacement } from "./Popover"
import { POPOVER_ALIGNMENT_CLASSES, POPOVER_PLACEMENT_CLASSES } from "./Popover"

export type TooltipFocusTrigger = "auto" | "always" | "never"

export interface TooltipProps {
  readonly children?: ReactNode
  readonly anchorRef?: RefObject<HTMLElement | null>
  readonly content: ReactNode
  readonly placement?: PopoverPlacement
  readonly alignment?: PopoverAlignment
  readonly delay?: number
  readonly hideDelay?: number
  readonly focusTrigger?: TooltipFocusTrigger
  readonly isEnabled?: boolean
  readonly onOpenChange?: (isOpen: boolean) => void
  readonly isOpen?: boolean
  readonly isDefaultOpen?: boolean
  readonly id?: string
  readonly className?: string
  readonly "data-testid"?: string
}

type TriggerProps = {
  readonly "aria-describedby"?: string
  readonly onMouseEnter?: (event: MouseEvent<HTMLElement>) => void
  readonly onMouseLeave?: (event: MouseEvent<HTMLElement>) => void
  readonly onFocus?: (event: FocusEvent<HTMLElement>) => void
  readonly onBlur?: (event: FocusEvent<HTMLElement>) => void
  readonly onKeyDown?: (event: KeyboardEvent<HTMLElement>) => void
  readonly tabIndex?: number
}

function isFocusableElement(element: ReactElement): boolean {
  const type = element.type
  if (type === "button" || type === "a" || type === "input" || type === "select" || type === "textarea") return true
  const props = element.props as { tabIndex?: number; href?: string; role?: string }
  return props.tabIndex !== undefined || props.href !== undefined || props.role === "button"
}

export function Tooltip({
  children,
  anchorRef,
  content,
  placement = "above",
  alignment = "center",
  delay = 0,
  hideDelay = 0,
  focusTrigger = "auto",
  isEnabled = true,
  onOpenChange,
  isOpen: controlledOpen,
  isDefaultOpen = false,
  id,
  className,
  "data-testid": testId,
}: TooltipProps) {
  const generatedId = useId()
  const tooltipId = id ?? generatedId
  const [internalOpen, setInternalOpen] = useState(isDefaultOpen)
  const isControlled = controlledOpen !== undefined
  const open = isControlled ? controlledOpen : internalOpen
  const openRef = useRef(open)
  const callbackRef = useRef(onOpenChange)
  const showTimer = useRef<ReturnType<typeof setTimeout> | null>(null)
  const hideTimer = useRef<ReturnType<typeof setTimeout> | null>(null)
  openRef.current = open
  callbackRef.current = onOpenChange

  const setOpen = (next: boolean) => {
    if (!isControlled) setInternalOpen(next)
    callbackRef.current?.(next)
  }
  const setOpenRef = useRef(setOpen)
  setOpenRef.current = setOpen

  const clearTimers = () => {
    if (showTimer.current !== null) clearTimeout(showTimer.current)
    if (hideTimer.current !== null) clearTimeout(hideTimer.current)
    showTimer.current = null
    hideTimer.current = null
  }
  const scheduleShow = useCallback(() => {
    if (!isEnabled) return
    if (hideTimer.current !== null) clearTimeout(hideTimer.current)
    if (openRef.current) return
    if (delay <= 0) setOpenRef.current(true)
    else showTimer.current = setTimeout(() => setOpenRef.current(true), delay)
  }, [delay, isEnabled])
  const scheduleHide = useCallback(() => {
    if (!isEnabled) return
    if (showTimer.current !== null) clearTimeout(showTimer.current)
    if (hideDelay <= 0) setOpenRef.current(false)
    else hideTimer.current = setTimeout(() => setOpenRef.current(false), hideDelay)
  }, [hideDelay, isEnabled])

  useEffect(() => clearTimers, [])

  const attachExternalAnchor = useCallback((anchor: HTMLElement | null) => {
    if (!anchor) return undefined
    const previousDescribedBy = anchor.getAttribute("aria-describedby")
    const previousExpanded = anchor.getAttribute("aria-expanded")
    anchor.setAttribute("aria-describedby", mergeDescribedBy(previousDescribedBy, tooltipId) ?? tooltipId)
    const onMouseEnter = () => scheduleShow()
    const onMouseLeave = () => scheduleHide()
    const onFocus = () => {
      if (focusTrigger !== "never") scheduleShow()
    }
    const onBlur = () => scheduleHide()
    if (isEnabled) {
      anchor.addEventListener("mouseenter", onMouseEnter)
      anchor.addEventListener("mouseleave", onMouseLeave)
      if (focusTrigger !== "never") anchor.addEventListener("focus", onFocus)
      anchor.addEventListener("blur", onBlur)
    }
    return () => {
      if (previousDescribedBy) anchor.setAttribute("aria-describedby", previousDescribedBy)
      else anchor.removeAttribute("aria-describedby")
      if (previousExpanded) anchor.setAttribute("aria-expanded", previousExpanded)
      else anchor.removeAttribute("aria-expanded")
      anchor.removeEventListener("mouseenter", onMouseEnter)
      anchor.removeEventListener("mouseleave", onMouseLeave)
      if (focusTrigger !== "never") anchor.removeEventListener("focus", onFocus)
      anchor.removeEventListener("blur", onBlur)
    }
  }, [focusTrigger, isEnabled, scheduleHide, scheduleShow, tooltipId])

  useEffect(() => {
    if (!anchorRef) return
    return attachExternalAnchor(anchorRef.current)
  }, [anchorRef, attachExternalAnchor, focusTrigger, isEnabled, tooltipId])

  const addTriggerHandlers = (element: ReactElement): ReactElement => {
    const childProps = element.props as TriggerProps
    const canFocus = isFocusableElement(element)
    const focusEnabled = focusTrigger === "always" || (focusTrigger === "auto" && canFocus)
    return cloneElement(element as ReactElement<Record<string, unknown>>, {
      "aria-describedby": mergeDescribedBy(childProps["aria-describedby"], tooltipId),
      onMouseEnter: (event: MouseEvent<HTMLElement>) => {
        childProps.onMouseEnter?.(event)
        if (!event.defaultPrevented) scheduleShow()
      },
      onMouseLeave: (event: MouseEvent<HTMLElement>) => {
        childProps.onMouseLeave?.(event)
        if (!event.defaultPrevented) scheduleHide()
      },
      onFocus: (event: FocusEvent<HTMLElement>) => {
        childProps.onFocus?.(event)
        if (focusEnabled && !event.defaultPrevented) scheduleShow()
      },
      onBlur: (event: FocusEvent<HTMLElement>) => {
        childProps.onBlur?.(event)
        if (!event.defaultPrevented) scheduleHide()
      },
    })
  }

  let trigger: ReactNode = children
  if (!anchorRef && isValidElement(children)) {
    trigger = addTriggerHandlers(children)
  } else if (!anchorRef && children !== undefined && children !== null) {
    trigger = (
      <output
        tabIndex={0}
        aria-describedby={tooltipId}
        onMouseEnter={scheduleShow}
        onMouseLeave={scheduleHide}
        onFocus={focusTrigger === "never" ? undefined : scheduleShow}
        onBlur={scheduleHide}
      >
        {children}
      </output>
    )
  }

  return (
    <section className="relative inline-flex">
      {trigger}
      <aside
        id={tooltipId}
        role="tooltip"
        data-open={open ? "true" : "false"}
        data-testid={testId}
        className={classNames(
          open ? "block" : "sr-only",
          "absolute z-50 max-w-xs rounded-md bg-inverted px-2 py-1 text-xs text-primary shadow-md",
          POPOVER_PLACEMENT_CLASSES[placement],
          POPOVER_ALIGNMENT_CLASSES[alignment],
          className,
        )}
      >
        {content}
      </aside>
    </section>
  )
}

Tooltip.displayName = "AstryxTooltip"

export const AstryxTooltip = Tooltip
