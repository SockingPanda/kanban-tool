/** 浏览器可能在 pagehide 之前清空 activeElement；保留最后一次用户聚焦的控件。 */
export function installPageFocusRecovery(page: Window): () => void {
  const document = page.document
  let focused: HTMLElement | null = null
  let retained: HTMLElement | null = null
  let frame: number | null = null
  const focusIn = (event: FocusEvent) => {
    focused = event.target instanceof HTMLElement && event.target !== document.body ? event.target : null
  }
  const pointerDown = (event: PointerEvent) => {
    if (!(event.target instanceof Node) || !focused?.contains(event.target)) focused = null
  }
  const keyDown = (event: KeyboardEvent) => { if (event.key === 'Tab') focused = null }
  const pageHide = (event: PageTransitionEvent) => {
    if (frame !== null) page.cancelAnimationFrame(frame)
    frame = null
    retained = event.persisted ? focused : null
  }
  const pageShow = (event: PageTransitionEvent) => {
    const target = retained
    retained = null
    if (!event.persisted || !target) return
    frame = page.requestAnimationFrame(() => {
      frame = null
      // 原控件仍存在且没有新的焦点时才恢复；不覆盖浏览器或用户已经选择的控件。
      if (target.isConnected && (document.activeElement === document.body || document.activeElement === document.documentElement)) {
        target.focus({ preventScroll: true })
      }
    })
  }
  document.addEventListener('focusin', focusIn)
  document.addEventListener('pointerdown', pointerDown, true)
  document.addEventListener('keydown', keyDown, true)
  page.addEventListener('pagehide', pageHide)
  page.addEventListener('pageshow', pageShow)
  return () => {
    if (frame !== null) page.cancelAnimationFrame(frame)
    document.removeEventListener('focusin', focusIn)
    document.removeEventListener('pointerdown', pointerDown, true)
    document.removeEventListener('keydown', keyDown, true)
    page.removeEventListener('pagehide', pageHide)
    page.removeEventListener('pageshow', pageShow)
    focused = null
    retained = null
  }
}
