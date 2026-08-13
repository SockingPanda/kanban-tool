import { useRef, type PointerEvent, type KeyboardEvent } from "react"

import {
  SIDEBAR_WIDTH_STEP_MAX,
  SIDEBAR_WIDTH_STEP_MIN,
  SIDEBAR_WIDTH_STEP_REM,
  normalizeSidebarWidthStep,
  shiftSidebarWidthStep,
  sidebarWidthRem,
} from "../../lib/preferences"
import styles from "./navigation.module.css"

export type SidebarResizeHandleProps = {
  readonly step: number
  readonly onStepChange: (step: number) => void
  readonly onReset: () => void
  readonly label?: string
}

function rootFontSize(): number {
  if (typeof document === "undefined" || typeof window === "undefined") return 16
  const value = Number.parseFloat(window.getComputedStyle(document.documentElement).fontSize)
  return Number.isFinite(value) && value > 0 ? value : 16
}

export function sidebarWidthStepForPointerDelta(startStep: number, deltaX: number, fontSize = 16): number {
  const pixelsPerStep = SIDEBAR_WIDTH_STEP_REM * (Number.isFinite(fontSize) && fontSize > 0 ? fontSize : 16)
  return shiftSidebarWidthStep(startStep, Math.round(deltaX / pixelsPerStep))
}

function keyboardStep(currentStep: number, key: string): number | null {
  if (key === "ArrowLeft") return shiftSidebarWidthStep(currentStep, -1)
  if (key === "ArrowRight") return shiftSidebarWidthStep(currentStep, 1)
  if (key === "Home") return SIDEBAR_WIDTH_STEP_MIN
  if (key === "End") return SIDEBAR_WIDTH_STEP_MAX
  return null
}

export function SidebarResizeHandle({ step, onStepChange, onReset, label = "Resize projects sidebar" }: SidebarResizeHandleProps) {
  const dragRef = useRef<{ readonly pointerId: number; readonly startX: number; readonly startStep: number } | null>(null)
  const normalizedStep = normalizeSidebarWidthStep(step)

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const nextStep = keyboardStep(normalizedStep, event.key)
    if (nextStep === null) return
    event.preventDefault()
    onStepChange(nextStep)
  }

  const onPointerDown = (event: PointerEvent<HTMLDivElement>) => {
    if (!event.isPrimary || event.button !== 0) return
    event.currentTarget.setPointerCapture(event.pointerId)
    dragRef.current = { pointerId: event.pointerId, startX: event.clientX, startStep: normalizedStep }
  }

  const onPointerMove = (event: PointerEvent<HTMLDivElement>) => {
    const drag = dragRef.current
    if (drag === null || drag.pointerId !== event.pointerId) return
    onStepChange(sidebarWidthStepForPointerDelta(drag.startStep, event.clientX - drag.startX, rootFontSize()))
  }

  const stopPointer = (event: PointerEvent<HTMLDivElement>) => {
    if (dragRef.current?.pointerId === event.pointerId) dragRef.current = null
  }

  return (
    <div
      className={styles.sidebarResizeHandle}
      role="separator"
      aria-label={label}
      aria-orientation="vertical"
      aria-valuemin={SIDEBAR_WIDTH_STEP_MIN}
      aria-valuemax={SIDEBAR_WIDTH_STEP_MAX}
      aria-valuenow={normalizedStep}
      aria-valuetext={sidebarWidthRem(normalizedStep)}
      data-testid="sidebar-resize-handle"
      data-sidebar-width-step={normalizedStep}
      tabIndex={0}
      onKeyDown={onKeyDown}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={stopPointer}
      onPointerCancel={stopPointer}
      onLostPointerCapture={stopPointer}
      onDoubleClick={onReset}
    />
  )
}

export default SidebarResizeHandle
