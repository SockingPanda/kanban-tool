import { useEffect, useMemo, useRef, useState } from "react"

import { Button } from "@astryxdesign/core/Button"
import { Card } from "@astryxdesign/core/Card"
import { Table, TableBody, TableCell, TableHeader, TableHeaderCell, TableRow } from "@astryxdesign/core/Table"
import { Theme } from "@astryxdesign/core/theme"
import { VStack } from "@astryxdesign/core/VStack"
import { neutralTheme } from "@astryxdesign/theme-neutral/built"

import styles from "./foundation.module.css"
import { useWebRuntime } from "./lib/runtime-context"

type ThemeMode = "light" | "dark"
type Locale = "zh" | "en"

const rows = [
  { signal: "task.created", owner: "local", state: "ready" },
  { signal: "task.claimed", owner: "worker-01", state: "running" },
]

function platformFeatures() {
  const popover = typeof HTMLElement !== "undefined" && "popover" in HTMLElement.prototype
  const anchorPositioning =
    typeof CSS !== "undefined" && CSS.supports("anchor-name: --astryx-anchor")

  return `popover: ${popover ? "supported" : "unsupported"}; anchor positioning: ${anchorPositioning ? "supported" : "unsupported"}`
}

export default function App() {
  const runtime = useWebRuntime()
  const [mode, setMode] = useState<ThemeMode>("light")
  const [locale, setLocale] = useState<Locale>("zh")
  const [query, setQuery] = useState("")
  const [isOverlayOpen, setOverlayOpen] = useState(false)
  const overlayRef = useRef<HTMLDialogElement>(null)
  const overlayCloseRef = useRef<HTMLButtonElement>(null)
  const overlayTriggerRef = useRef<HTMLButtonElement>(null)
  const features = useMemo(platformFeatures, [])

  useEffect(() => {
    const dialog = overlayRef.current
    if (!dialog) return

    if (isOverlayOpen) {
      if (!dialog.open) dialog.showModal()
      overlayCloseRef.current?.focus()
    } else if (dialog.open) {
      dialog.close()
    }
  }, [isOverlayOpen])

  useEffect(() => {
    if (!isOverlayOpen) return
    const dialog = overlayRef.current
    if (!dialog) return

    const handleTab = (event: KeyboardEvent) => {
      if (event.key !== "Tab") return

      const focusable = [...dialog.querySelectorAll<HTMLElement>("button, input, select, textarea, a[href]")].filter(
        (element) => !element.hasAttribute("disabled") && element.tabIndex >= 0,
      )
      if (focusable.length === 0) return

      const currentIndex = focusable.indexOf(document.activeElement as HTMLElement)
      const nextIndex = event.shiftKey
        ? currentIndex <= 0
          ? focusable.length - 1
          : currentIndex - 1
        : currentIndex === -1 || currentIndex === focusable.length - 1
          ? 0
          : currentIndex + 1

      event.preventDefault()
      focusable[nextIndex]?.focus()
    }

    // showModal() 提供背景 inert 语义；这个守卫处理部分引擎退回
    // document.body 的情况，确保 Tab 始终留在 top layer 内。
    document.addEventListener("keydown", handleTab, true)
    return () => document.removeEventListener("keydown", handleTab, true)
  }, [isOverlayOpen])

  useEffect(() => {
    const dialog = overlayRef.current
    if (!dialog) return

    const handleCancel = (event: Event) => {
      event.preventDefault()
      setOverlayOpen(false)
    }
    const handleClose = () => {
      setOverlayOpen(false)
      overlayTriggerRef.current?.focus()
    }

    dialog.addEventListener("cancel", handleCancel)
    dialog.addEventListener("close", handleClose)
    return () => {
      dialog.removeEventListener("cancel", handleCancel)
      dialog.removeEventListener("close", handleClose)
    }
  }, [])

  const isEnglish = locale === "en"
  const longCopy = isEnglish
    ? "This operator console keeps the local canonical fact visible while persistent SSE carries durable invalidation to every surface. The foundation lab intentionally favors calm density, explicit state, and predictable focus over a decorative dashboard mosaic."
    : "这个操作台保持本地规范事实可见，并以持久化 SSE 将可重建的失效事件送到每个界面。基础实验室刻意采用冷静的高密度布局、明确的状态和可预测的焦点，而不是装饰性的仪表盘拼贴。"

  useEffect(() => {
    document.documentElement.lang = isEnglish ? "en" : "zh-CN"
  }, [isEnglish])

  useEffect(() => {
    document.querySelector('meta[name="theme-color"]')?.setAttribute("content", mode === "light" ? "#f1f1f1" : "#1b1b1b")
  }, [mode])

  return (
    <Theme theme={neutralTheme} mode={mode}>
      <a className={styles.skipLink} href="#main-content">
        跳转到主要内容
      </a>
      <main
        id="main-content"
        className={styles.shell}
        tabIndex={-1}
        data-runtime-api-base-url={runtime.apiBaseUrl}
        data-runtime-actor={runtime.actor}
        data-runtime-default-board={runtime.defaultBoard}
        data-runtime-server-version={runtime.serverVersion}
        data-runtime-protocol-version={runtime.protocolVersion}
        data-runtime-web-build-id={runtime.webBuildId}
        data-runtime-web-base-path={runtime.webBasePath}
      >
        <header className={styles.header}>
          <div>
            <p className={styles.eyebrow}>KANBAN TOOL / ASTRYX FOUNDATION</p>
            <h1>Astryx foundation lab</h1>
            <p className={styles.lede} data-testid="long-copy">
              {longCopy}
            </p>
          </div>
          <div className={styles.actions} role="group" aria-label="Foundation controls">
            <Button
              label="切换主题"
              variant="secondary"
              onClick={() => setMode((current) => (current === "light" ? "dark" : "light"))}
              data-testid="theme-toggle"
            />
            <Button
              label="切换语言"
              variant="ghost"
              onClick={() => setLocale((current) => (current === "zh" ? "en" : "zh"))}
              data-testid="locale-toggle"
            />
          </div>
        </header>

        <section className={styles.signalStrip} aria-label="Platform feature detection">
          <span className={styles.signalLabel}>Runtime features</span>
          <span data-testid="platform-features">{features}</span>
        </section>

        <VStack className={styles.stack} data-testid="astryx-vstack">
          <Card className={styles.card} padding={4} data-testid="astryx-card">
            <div className={styles.cardHeader}>
              <div>
                <p className={styles.kicker}>PUBLIC SEAM / 01</p>
                <h2>Command input</h2>
              </div>
              <span className={styles.status}>READY</span>
            </div>
            <div className={styles.formRow}>
              <div className={styles.field}>
                <label className={styles.fieldLabel} htmlFor="event-query">
                  Search canonical events
                </label>
                <input
                  id="event-query"
                  type="search"
                  name="eventQuery"
                  className={styles.textInput}
                  value={query}
                  onChange={(event) => setQuery(event.currentTarget.value)}
                  autoComplete="off"
                  placeholder="task.created…"
                  aria-describedby="event-query-description"
                  data-testid="foundation-text-input"
                />
                <p id="event-query-description" className={styles.fieldDescription}>
                  Static CSS and typed state only.
                </p>
              </div>
              <Button label="Inspect signal" variant="primary" data-testid="astryx-button" />
            </div>
          </Card>

          <Card className={styles.card} padding={4} data-testid="astryx-table-card">
            <div className={styles.cardHeader}>
              <div>
                <p className={styles.kicker}>PUBLIC SEAM / 02</p>
                <h2>Event queue</h2>
              </div>
              <span className={styles.muted}>2 visible rows</span>
            </div>
            <div className={styles.tableWrap} data-testid="astryx-table">
              <Table<Record<string, unknown>> dividers="rows" textOverflow="wrap">
                <TableHeader>
                  <TableRow isHeaderRow>
                    <TableHeaderCell>Signal</TableHeaderCell>
                    <TableHeaderCell>Owner</TableHeaderCell>
                    <TableHeaderCell>State</TableHeaderCell>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {rows.map((row) => (
                    <TableRow key={row.signal}>
                      <TableCell>{row.signal}</TableCell>
                      <TableCell>{row.owner}</TableCell>
                      <TableCell>{row.state}</TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </div>
          </Card>

          <Card className={styles.card} padding={4} data-testid="overlay-card">
            <div className={styles.cardHeader}>
              <div>
                <p className={styles.kicker}>PUBLIC SEAM / 03</p>
                <h2>Overlay behavior</h2>
              </div>
              <Button
                ref={overlayTriggerRef}
                label="Open overlay"
                variant="secondary"
                onClick={() => setOverlayOpen(true)}
              />
            </div>
            <p className={styles.muted}>
              The modal uses a semantic HTML fallback because Astryx Dialog 0.3.0 writes runtime
              positioning styles. Packaged WebKitGTK remains a separate host-level verification target.
            </p>
          </Card>
        </VStack>

        <dialog
          ref={overlayRef}
          className={styles.overlay}
          aria-labelledby="overlay-title"
          aria-describedby="overlay-description"
          data-testid="foundation-dialog"
        >
          <div className={styles.overlayPanel}>
            <h2 id="overlay-title">Overlay verification</h2>
            <div className={styles.dialogBody}>
              <p id="overlay-description">Astryx overlay seam</p>
              <label className={styles.fieldLabel} htmlFor="overlay-focus-probe">
                Focus probe
              </label>
              <input
                id="overlay-focus-probe"
                name="overlayFocusProbe"
                className={styles.textInput}
                defaultValue="native modal"
                readOnly
                autoComplete="off"
                data-testid="overlay-focus-probe"
              />
              <button
                ref={overlayCloseRef}
                className={styles.nativeButton}
                type="button"
                onClick={() => setOverlayOpen(false)}
              >
                Close overlay
              </button>
            </div>
          </div>
        </dialog>
      </main>
    </Theme>
  )
}
