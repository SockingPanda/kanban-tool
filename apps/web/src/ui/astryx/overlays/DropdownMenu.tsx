import {
  useEffect,
  useId,
  useRef,
  useState,
  type KeyboardEvent,
  type ReactNode,
  type Ref,
} from "react"

import {
  classNames,
  isTopOverlay,
  supportsNativePopover,
  useOverlayInteraction,
} from "./overlay-runtime"
import {
  POPOVER_ALIGNMENT_CLASSES,
  POPOVER_PLACEMENT_CLASSES,
  type PopoverAlignment,
  type PopoverPlacement,
} from "./Popover"

export interface DropdownMenuItemData {
  readonly label: string
  readonly onClick?: () => void
  readonly isDisabled?: boolean
  readonly icon?: ReactNode
}

export interface DropdownMenuDivider {
  readonly type: "divider"
}

export interface DropdownMenuSection {
  readonly type: "section"
  readonly title?: string
  readonly items: readonly DropdownMenuItemData[]
}

export type DropdownMenuOption = DropdownMenuItemData | DropdownMenuDivider | DropdownMenuSection

export interface DropdownMenuButtonProps {
  readonly label?: string
  readonly icon?: ReactNode
  readonly variant?: "primary" | "secondary" | "ghost"
  readonly size?: "sm" | "md" | "lg"
  readonly isDisabled?: boolean
  readonly isIconOnly?: boolean
  readonly className?: string
  readonly "aria-label"?: string
}

export interface DropdownMenuProps {
  readonly button?: DropdownMenuButtonProps
  readonly items: readonly DropdownMenuOption[]
  readonly isMenuOpen?: boolean
  readonly isOpen?: boolean
  readonly onOpenChange?: (isOpen: boolean) => void
  readonly onClick?: () => void
  readonly hasChevron?: boolean
  readonly placement?: PopoverPlacement
  readonly alignment?: PopoverAlignment
  readonly className?: string
  readonly id?: string
  readonly "data-testid"?: string
  readonly triggerRef?: Ref<HTMLButtonElement>
  readonly children?: (item: DropdownMenuItemData) => ReactNode
}

export const DROPDOWN_MENU_PLACEMENT_CLASSES = POPOVER_PLACEMENT_CLASSES
export const DROPDOWN_MENU_ALIGNMENT_CLASSES = POPOVER_ALIGNMENT_CLASSES
export const DROPDOWN_MENU_BUTTON_SIZE_CLASSES: Readonly<Record<NonNullable<DropdownMenuButtonProps["size"]>, string>> = Object.freeze({
  sm: "text-xs",
  md: "text-sm",
  lg: "text-base",
})

function itemButtonClass(disabled: boolean): string {
  return classNames(
    "flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-start text-sm",
    disabled ? "cursor-not-allowed opacity-50" : "hover:bg-muted focus-visible:bg-muted",
  )
}

export function DropdownMenu({
  button = { label: "Menu" },
  items,
  isMenuOpen,
  isOpen,
  onOpenChange,
  onClick,
  hasChevron = true,
  placement = "below",
  alignment = "start",
  className,
  id,
  "data-testid": testId,
  triggerRef,
  children: renderItem,
}: DropdownMenuProps) {
  const generatedMenuId = useId()
  const menuId = id ?? generatedMenuId
  const triggerElementRef = useRef<HTMLButtonElement | null>(null)
  const menuRef = useRef<HTMLElement | null>(null)
  const restoreFocusRef = useRef<HTMLElement | null>(null)
  const [internalOpen, setInternalOpen] = useState(false)
  const controlledOpen = isMenuOpen ?? isOpen
  const controlled = controlledOpen !== undefined
  const open = controlled ? controlledOpen : internalOpen
  const openRef = useRef(open)
  const callbackRef = useRef(onOpenChange)
  const lastHideRef = useRef(0)
  const skipRestoreFocusRef = useRef(false)
  const focusOnOpenRef = useRef<"first" | "last">("first")
  const typeaheadRef = useRef({ value: "", at: 0 })
  openRef.current = open
  callbackRef.current = onOpenChange

  const assignTriggerRef = (node: HTMLButtonElement | null) => {
    triggerElementRef.current = node
    if (typeof triggerRef === "function") triggerRef(node)
    else if (triggerRef) triggerRef.current = node
  }

  const setOpen = (next: boolean) => {
    if (!controlled) setInternalOpen(next)
    callbackRef.current?.(next)
  }
  const openMenu = (focus: "first" | "last" = "first") => {
    skipRestoreFocusRef.current = false
    focusOnOpenRef.current = focus
    setOpen(true)
  }
  const close = () => {
    if (!openRef.current) return
    lastHideRef.current = Date.now()
    setOpen(false)
  }
  const toggle = () => {
    if (button.isDisabled || Date.now() - lastHideRef.current < 50) return
    onClick?.()
    if (openRef.current) close()
    else openMenu()
  }
  const closeRef = useRef(close)
  closeRef.current = close

  useOverlayInteraction(menuRef, {
    enabled: open,
    restoreFocusRef,
    shouldRestoreFocus: () => !skipRestoreFocusRef.current,
    onEscape: () => {
      if (menuRef.current && isTopOverlay(menuRef.current)) close()
    },
    trapFocus: false,
    autoFocus: false,
  })

  useEffect(() => {
    const menu = menuRef.current
    if (!menu) return
    // Keep menu geometry DOM-contained; native popover support is recorded for
    // diagnostics because its top-layer positioning cannot use our static map.
    menu.dataset.popoverSupport = supportsNativePopover(menu) ? "native" : "fallback"
  }, [])

  useEffect(() => {
    if (!open) return
    const onPointerDown = (event: PointerEvent) => {
      const menu = menuRef.current
      const trigger = triggerElementRef.current
      if (!menu || !isTopOverlay(menu)) return
      const target = event.target
      if (target instanceof Node && !menu.contains(target) && !trigger?.contains(target)) closeRef.current()
    }
    document.addEventListener("pointerdown", onPointerDown, true)
    return () => document.removeEventListener("pointerdown", onPointerDown, true)
  }, [open])

  useEffect(() => {
    if (!open) return
    const focusMenuItem = () => {
      const menu = menuRef.current
      if (!menu) return
      const targets = Array.from(menu.querySelectorAll<HTMLElement>("[role=menuitem]:not([aria-disabled=\"true\"])"))
      const target = focusOnOpenRef.current === "last" ? targets[targets.length - 1] : targets[0]
      target?.focus({ preventScroll: true })
      if (!target) menu.focus({ preventScroll: true })
    }
    const frame = typeof requestAnimationFrame === "function"
      ? requestAnimationFrame(focusMenuItem)
      : window.setTimeout(focusMenuItem, 0)
    return () => {
      if (typeof frame === "number") {
        if (typeof cancelAnimationFrame === "function") cancelAnimationFrame(frame)
        else window.clearTimeout(frame)
      }
    }
  }, [open])

  const focusableItems = () => Array.from(menuRef.current?.querySelectorAll<HTMLElement>("[role=menuitem]:not([aria-disabled=\"true\"])") ?? [])

  const handleMenuKeyDown = (event: KeyboardEvent<HTMLElement>) => {
    const menu = menuRef.current
    if (!menu || !isTopOverlay(menu)) return
    const targets = focusableItems()
    const active = document.activeElement instanceof HTMLElement ? document.activeElement : null
    const index = active ? targets.indexOf(active) : -1
    if (event.key === "Escape") {
      event.preventDefault()
      close()
    } else if (event.key === "Tab") {
      skipRestoreFocusRef.current = true
      close()
    } else if (event.key === "ArrowDown" || event.key === "ArrowRight") {
      event.preventDefault()
      targets[(index + 1 + targets.length) % targets.length]?.focus({ preventScroll: true })
    } else if (event.key === "ArrowUp" || event.key === "ArrowLeft") {
      event.preventDefault()
      targets[(index - 1 + targets.length) % targets.length]?.focus({ preventScroll: true })
    } else if (event.key === "Home") {
      event.preventDefault()
      targets[0]?.focus({ preventScroll: true })
    } else if (event.key === "End") {
      event.preventDefault()
      targets[targets.length - 1]?.focus({ preventScroll: true })
    } else if (event.key.length === 1 && !event.ctrlKey && !event.metaKey && !event.altKey) {
      const now = Date.now()
      const previous = typeaheadRef.current
      const value = now - previous.at < 500 ? `${previous.value}${event.key.toLocaleLowerCase()}` : event.key.toLocaleLowerCase()
      typeaheadRef.current = { value, at: now }
      const matches = targets.filter((target) => target.textContent?.trim().toLocaleLowerCase().startsWith(value))
      const fallbackMatches = matches.length > 0
        ? matches
        : targets.filter((target) => target.textContent?.trim().toLocaleLowerCase().startsWith(event.key.toLocaleLowerCase()))
      const next = fallbackMatches.find((target) => targets.indexOf(target) > index) ?? fallbackMatches[0]
      next?.focus({ preventScroll: true })
    }
  }

  const renderOptions = () => {
    let actionIndex = 0
    return items.map((item, index) => {
      if ("type" in item) {
        if (item.type === "divider") {
          return <li key={`divider-${index}`} role="separator" className="my-1 border-t border-border" />
        }
        if (item.type === "section") {
          return (
            <li key={`section-${index}`} role="none">
              <menu role="group" aria-label={item.title}>
                {item.title ? <p className="px-2 py-1 text-xs font-medium text-secondary">{item.title}</p> : null}
                {item.items.map((entry) => renderAction(entry, actionIndex++))}
              </menu>
            </li>
          )
        }
      }
      return renderAction(item, actionIndex++)
    })
  }

  const renderAction = (item: DropdownMenuItemData, index: number) => {
    const disabled = Boolean(item.isDisabled || !item.onClick)
    return (
      <li key={`${item.label}-${index}`} role="none">
        <button
          type="button"
          role="menuitem"
          tabIndex={-1}
          aria-disabled={disabled ? "true" : undefined}
          disabled={disabled}
          className={itemButtonClass(disabled)}
          onClick={() => {
            if (!disabled) {
              item.onClick?.()
              close()
            }
          }}
        >
          {item.icon ?? null}
          {renderItem ? renderItem(item) : item.label}
        </button>
      </li>
    )
  }

  const label = button.label ?? ""
  const triggerLabel = button["aria-label"] ?? (label || undefined)

  return (
    <section className="relative inline-flex">
      <button
        ref={assignTriggerRef}
        type="button"
        aria-label={triggerLabel}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={menuId}
        disabled={button.isDisabled}
        className={classNames(
          button.isIconOnly ? "inline-flex items-center justify-center" : "inline-flex items-center gap-2",
          "rounded-md px-2 py-1.5 font-medium focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent",
          DROPDOWN_MENU_BUTTON_SIZE_CLASSES[button.size ?? "md"],
          button.variant === "primary" ? "bg-accent-bg text-on-accent hover:bg-accent-bg" : button.variant === "secondary" ? "border border-border-strong bg-surface text-primary hover:bg-muted" : "text-primary hover:bg-muted",
          button.className,
        )}
        onClick={toggle}
        onKeyDown={(event) => {
          if (!open && (event.key === "ArrowDown" || event.key === "Enter" || event.key === " ")) {
            event.preventDefault()
            openMenu()
          } else if (!open && event.key === "ArrowUp") {
            event.preventDefault()
            openMenu("last")
          }
        }}
      >
        {button.icon ?? null}
        {!button.isIconOnly ? label : null}
        {hasChevron && !button.isIconOnly ? <i aria-hidden="true" className="text-xs">⌄</i> : null}
      </button>
      <menu
        ref={menuRef}
        id={menuId}
        role="menu"
        aria-label={label}
        aria-hidden={!open}
        tabIndex={-1}
        data-open={open ? "true" : "false"}
        data-testid={testId}
        className={classNames(
          open ? "block" : "hidden",
          "absolute z-40 min-w-40 rounded-lg border border-border bg-popover p-1 text-primary shadow-lg",
          POPOVER_PLACEMENT_CLASSES[placement],
          POPOVER_ALIGNMENT_CLASSES[alignment],
          className,
        )}
        onKeyDown={handleMenuKeyDown}
      >
        {renderOptions()}
      </menu>
    </section>
  )
}

DropdownMenu.displayName = "AstryxDropdownMenu"

export const AstryxDropdownMenu = DropdownMenu
