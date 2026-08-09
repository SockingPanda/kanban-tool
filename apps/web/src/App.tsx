import { useCallback, useEffect, useRef, useState } from "react"
import { InternationalizationProvider } from "@astryxdesign/core/i18n"
import { Theme } from "@astryxdesign/core/theme"
import { neutralTheme } from "@astryxdesign/theme-neutral/built"

import { ProductShell } from "./ProductShell"
import { BoardLive } from "./features/board/BoardLive"
import type { SyncTelemetryEntry } from "./lib/sync"
import { usePreferences } from "./lib/use-preferences"
import { PreferencesProvider } from "./lib/preferences-provider"
import { useAppRouter } from "./lib/router"
import { useWebRuntime } from "./lib/runtime-context"
import { astryxMessages, astryxOverrides } from "./lib/i18n"

const explorerInvalidationTelemetry = new Set([
  "event-applied",
  "recovery-complete",
  "poll-complete",
  "poll-boundary-complete",
  "protocol-anomaly",
  "isolation-anomaly",
])

function RuntimeThemedShell() {
  const runtime = useWebRuntime()
  const preferences = usePreferences()
  const router = useAppRouter({
    basePath: runtime.webBasePath,
    defaultBoard: runtime.defaultBoard,
  })
  const boardRoute = router.route.kind === "home" || router.route.kind === "board" ? router.route : null
  const liveBoardVisible = boardRoute?.kind === "home"
    || (boardRoute?.kind === "board" && (boardRoute.view === undefined || boardRoute.view === "board"))
  const sessionKey = boardRoute === null
    ? "none"
    : `${runtime.apiBaseUrl}\u0000${runtime.webBasePath}\u0000${runtime.webBuildId}\u0000${boardRoute.kind === "board" ? boardRoute.boardSlug : ""}`
  const sessionKeyRef = useRef(sessionKey)
  sessionKeyRef.current = sessionKey
  const [sessionState, setSessionState] = useState<{ readonly key: string; readonly revision: number }>(() => ({
    key: sessionKey,
    revision: 0,
  }))

  useEffect(() => {
    setSessionState((current) => current.key === sessionKey ? current : { key: sessionKey, revision: 0 })
  }, [sessionKey])

  const onSessionTelemetry = useCallback((entry: SyncTelemetryEntry) => {
    if (!explorerInvalidationTelemetry.has(entry.type)) return
    setSessionState((current) => {
      const key = sessionKeyRef.current
      if (current.key !== key) return current
      return {
        key,
        revision: current.revision + 1,
      }
    })
  }, [])

  return (
    <InternationalizationProvider locale={preferences.locale} messages={astryxMessages} overrides={astryxOverrides}>
      <Theme theme={neutralTheme} mode={preferences.theme}>
        <ProductShell
          runtime={runtime}
          route={router.route}
          canonicalBoardSlug={router.route.kind === "board" ? router.route.boardSlug : undefined}
          boundary={router.error ? "error" : undefined}
          error={router.error instanceof Error ? router.error.message : undefined}
          onNavigate={router.navigate}
          onRetry={() => window.location.reload()}
          invalidationRevision={sessionState.key === sessionKey ? sessionState.revision : 0}
        >
          {boardRoute ? (
            <BoardLive
              runtime={runtime}
              route={boardRoute}
              onNavigate={router.navigate}
              renderBoard={liveBoardVisible}
              onSessionTelemetry={onSessionTelemetry}
            />
          ) : null}
        </ProductShell>
      </Theme>
    </InternationalizationProvider>
  )
}

export default function App() {
  return (
    <PreferencesProvider>
      <RuntimeThemedShell />
    </PreferencesProvider>
  )
}
