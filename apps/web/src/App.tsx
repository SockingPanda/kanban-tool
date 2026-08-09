import { InternationalizationProvider } from "@astryxdesign/core/i18n"
import { Theme } from "@astryxdesign/core/theme"
import { neutralTheme } from "@astryxdesign/theme-neutral/built"
import { useEffect, useState, useSyncExternalStore } from "react"

import { ProductShell } from "./ProductShell"
import { BoardLive } from "./features/board/BoardLive"
import {
  hasActiveBoardSession,
  boardSessionRevision,
  reconnectActiveBoardSession,
  subscribeBoardSessions,
} from "./features/board/board-session-registry"
import type { CanonicalBoardSlug } from "./lib/board-slug"
import { routePath } from "./lib/router"
import { usePreferences } from "./lib/use-preferences"
import { PreferencesProvider } from "./lib/preferences-provider"
import { useAppRouter } from "./lib/router"
import { useWebRuntime } from "./lib/runtime-context"
import { astryxMessages, astryxOverrides } from "./lib/i18n"

function RuntimeThemedShell() {
  const runtime = useWebRuntime()
  const preferences = usePreferences()
  const router = useAppRouter({
    basePath: runtime.webBasePath,
    defaultBoard: runtime.defaultBoard,
  })
  const boardRoute = router.route.kind === "home" || router.route.kind === "board" ? router.route : null
  const routeBoardSlug = router.route.kind === "board" || router.route.kind === "health" || router.route.kind === "maintenance"
    ? router.route.boardSlug
    : null
  const [retainedBoardSlug, setRetainedBoardSlug] = useState<CanonicalBoardSlug | null>(() => routeBoardSlug)
  const sessionRevision = useSyncExternalStore(subscribeBoardSessions, boardSessionRevision, boardSessionRevision)

  useEffect(() => {
    if (routeBoardSlug !== null) setRetainedBoardSlug(routeBoardSlug)
  }, [routeBoardSlug])

  const activeBoardSlug = routeBoardSlug ?? retainedBoardSlug
  const sessionRoute = boardRoute ?? (activeBoardSlug === null
    ? null
    : {
        kind: "board" as const,
        boardSlug: activeBoardSlug,
        pathname: routePath({ kind: "board", boardSlug: activeBoardSlug }, { basePath: runtime.webBasePath }),
      })
  const sessionHidden = boardRoute === null
  // Session ownership notifications force this derived control state to rerender.
  const reconnectAvailable = sessionRevision >= 0 && hasActiveBoardSession(runtime, activeBoardSlug ?? undefined)

  return (
    <InternationalizationProvider locale={preferences.locale} messages={astryxMessages} overrides={astryxOverrides}>
      <Theme theme={neutralTheme} mode={preferences.theme}>
        <ProductShell
          runtime={runtime}
          route={router.route}
          canonicalBoardSlug={activeBoardSlug ?? undefined}
          boundary={router.error ? "error" : undefined}
          error={router.error instanceof Error ? router.error.message : undefined}
          onNavigate={router.navigate}
          onReconnect={reconnectAvailable ? () => reconnectActiveBoardSession(runtime, activeBoardSlug ?? undefined) : undefined}
          onRetry={() => window.location.reload()}
        >
          {sessionRoute ? (
            <div hidden={sessionHidden} aria-hidden={sessionHidden ? "true" : undefined}>
              <BoardLive runtime={runtime} route={sessionRoute} onNavigate={router.navigate} />
            </div>
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
