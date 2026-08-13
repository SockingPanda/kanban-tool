import {
  SIDEBAR_WIDTH_STEP_REM,
  shiftSidebarWidthStep,
} from "../../lib/preferences"

export function sidebarWidthStepForPointerDelta(startStep: number, deltaX: number, fontSize = 16): number {
  const pixelsPerStep = SIDEBAR_WIDTH_STEP_REM * (Number.isFinite(fontSize) && fontSize > 0 ? fontSize : 16)
  return shiftSidebarWidthStep(startStep, Math.round(deltaX / pixelsPerStep))
}
