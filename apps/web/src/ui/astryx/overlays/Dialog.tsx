import {
  forwardRef,
  useEffect,
  useId,
  useRef,
  useState,
  type MouseEvent,
  type AriaRole,
  type ReactNode,
  type Ref,
} from "react"

import { classNames, isTopOverlay, useOverlayInteraction } from "./overlay-runtime"
import {pickCspSafeDomProps} from "../dom-props"

export type DialogSize = "sm" | "md" | "lg" | "xl" | "full"
export type DialogPlacement = "center" | "top" | "bottom"

export const DIALOG_SIZE_CLASSES: Readonly<Record<DialogSize, string>> = Object.freeze({
  sm: "w-full max-w-sm",
  md: "w-full max-w-md",
  lg: "w-full max-w-lg",
  xl: "w-full max-w-2xl",
  full: "w-full max-w-full",
})

export const DIALOG_PLACEMENT_CLASSES: Readonly<Record<DialogPlacement, string>> = Object.freeze({
  center: "m-auto",
  top: "mt-8 mb-auto",
  bottom: "mt-auto mb-8",
})

type NativeDialogProps = {
  readonly id?: string
  readonly title?: string
  readonly role?: AriaRole
  readonly "aria-label"?: string
  readonly "aria-labelledby"?: string
  readonly "aria-describedby"?: string
  readonly onClick?: (event: MouseEvent<HTMLDialogElement>) => void
}

export interface DialogProps extends NativeDialogProps {
  readonly isOpen: boolean
  readonly onOpenChange: (isOpen: boolean) => void
  readonly children: ReactNode
  readonly size?: DialogSize
  readonly placement?: DialogPlacement
  /** Whether clicking the modal backdrop closes the dialog. */
  readonly closeOnBackdrop?: boolean
  readonly className?: string
  readonly returnFocusRef?: { current: HTMLElement | null }
  readonly initialFocusRef?: { current: HTMLElement | null }
  readonly onRuntimeError?: (error: unknown) => void
  readonly "data-testid"?: string
}

function setRef<T>(ref: Ref<T> | undefined, value: T | null): void {
  if (typeof ref === "function") ref(value)
  else if (ref) ref.current = value
}

function focusInitial(dialog: HTMLDialogElement, initialFocusRef: DialogProps["initialFocusRef"]): void {
  const explicit = initialFocusRef?.current
  if (explicit && dialog.contains(explicit)) {
    explicit.focus({ preventScroll: true })
    return
  }
  const target = dialog.querySelector<HTMLElement>("[data-autofocus]")
  if (target) {
    target.focus({ preventScroll: true })
    return
  }
  const first = dialog.querySelector<HTMLElement>(
    "a[href], area[href], button, input, select, textarea, summary, [contenteditable=\"true\"], [tabindex]:not([tabindex=\"-1\"])",
  )
  first?.focus({ preventScroll: true })
}

export const Dialog = forwardRef<HTMLDialogElement, DialogProps>(function Dialog(
  {
    isOpen,
    onOpenChange,
    children,
    size = "md",
    placement = "center",
    closeOnBackdrop = true,
    className,
    returnFocusRef,
    initialFocusRef,
    onRuntimeError,
    id,
    onClick,
    "aria-label": ariaLabel,
    "aria-labelledby": ariaLabelledBy,
    "data-testid": testId,
    ...props
  },
  forwardedRef,
) {
  const generatedId = useId()
  const dialogRef = useRef<HTMLDialogElement | null>(null)
  const restoreFocusRef = useRef<HTMLElement | null>(null)
  const callbackRef = useRef(onOpenChange)
  const runtimeErrorRef = useRef(onRuntimeError)
  const closingRef = useRef(false)
  const [nativeOpen, setNativeOpen] = useState(false)
  callbackRef.current = onOpenChange
  runtimeErrorRef.current = onRuntimeError

  useOverlayInteraction(dialogRef, {
    enabled: isOpen,
    restoreFocusRef: returnFocusRef ?? restoreFocusRef,
    onEscape: () => {
      const dialog = dialogRef.current
      if (dialog && isTopOverlay(dialog)) callbackRef.current(false)
    },
    trapFocus: true,
    autoFocus: false,
  })

  useEffect(() => {
    const dialog = dialogRef.current
    if (!dialog || typeof document === "undefined") return

    if (isOpen) {
      if (!dialog.open) {
        restoreFocusRef.current = returnFocusRef?.current ??
          (document.activeElement instanceof HTMLElement ? document.activeElement : null)
        if (typeof dialog.showModal !== "function") {
          const error = new Error("AstryxDialog requires HTMLDialogElement.showModal")
          setNativeOpen(false)
          runtimeErrorRef.current?.(error)
          callbackRef.current(false)
          return undefined
        }
        try {
          dialog.showModal()
          setNativeOpen(true)
        } catch (error) {
          setNativeOpen(false)
          runtimeErrorRef.current?.(error)
          callbackRef.current(false)
          return undefined
        }
      } else {
        setNativeOpen(true)
      }
      const frame = typeof requestAnimationFrame === "function"
        ? requestAnimationFrame(() => focusInitial(dialog, initialFocusRef))
        : window.setTimeout(() => focusInitial(dialog, initialFocusRef), 0)
      return () => {
        if (typeof frame === "number") {
          if (typeof cancelAnimationFrame === "function") cancelAnimationFrame(frame)
          else window.clearTimeout(frame)
        }
      }
    }

    if (dialog.open) {
      closingRef.current = true
      try {
        dialog.close()
        setNativeOpen(false)
      } catch (error) {
        setNativeOpen(true)
        runtimeErrorRef.current?.(error)
        callbackRef.current(true)
      }
      closingRef.current = false
    } else {
      setNativeOpen(false)
    }
    return undefined
  }, [initialFocusRef, isOpen, returnFocusRef])

  useEffect(() => {
    const dialog = dialogRef.current
    if (!dialog || !isOpen || !nativeOpen) return
    focusInitial(dialog, initialFocusRef)
  }, [initialFocusRef, isOpen, nativeOpen])

  useEffect(() => {
    const dialog = dialogRef.current
    if (!dialog || !isOpen) return

    const handleCancel = (event: Event) => {
      event.preventDefault()
      if (isTopOverlay(dialog)) callbackRef.current(false)
    }
    const handleClose = () => {
      setNativeOpen(false)
      if (!closingRef.current && isTopOverlay(dialog)) callbackRef.current(false)
    }
    dialog.addEventListener("cancel", handleCancel)
    dialog.addEventListener("close", handleClose)
    return () => {
      dialog.removeEventListener("cancel", handleCancel)
      dialog.removeEventListener("close", handleClose)
    }
  }, [isOpen])

  const dialogId = id ?? generatedId
  const handleClick = (event: MouseEvent<HTMLDialogElement>) => {
    onClick?.(event)
    if (closeOnBackdrop && event.target === event.currentTarget && isTopOverlay(event.currentTarget)) {
      callbackRef.current(false)
    }
  }

  return (
    <dialog
      {...pickCspSafeDomProps(props)}
      ref={(node) => {
        dialogRef.current = node
        setRef(forwardedRef, node)
      }}
      id={dialogId}
      aria-label={ariaLabel}
      aria-labelledby={ariaLabelledBy}
      aria-modal="true"
      data-open={nativeOpen ? "true" : "false"}
      data-testid={testId}
      tabIndex={-1}
      className={classNames(
        nativeOpen ? "flex" : "hidden",
        "fixed inset-0 z-50 max-h-full flex-col overflow-hidden rounded-lg border border-border bg-surface p-0 text-primary shadow-xl backdrop:bg-inverted",
        DIALOG_SIZE_CLASSES[size],
        DIALOG_PLACEMENT_CLASSES[placement],
        className,
      )}
      onClick={handleClick}
    >
      {children}
    </dialog>
  )
})

Dialog.displayName = "AstryxDialog"

export const AstryxDialog = Dialog
