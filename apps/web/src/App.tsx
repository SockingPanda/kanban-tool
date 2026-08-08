import { InternationalizationProvider } from "@astryxdesign/core/i18n"
import { Theme } from "@astryxdesign/core/theme"
import { neutralTheme } from "@astryxdesign/theme-neutral/built"

import { ProductShell } from "./ProductShell"
import { BoardFeatureRoute, type FeatureRoute } from "./features/BoardFeatureRoute"
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
        >
          {router.route.kind === "board" && router.route.view ? (
            <BoardFeatureRoute runtime={runtime} route={router.route as FeatureRoute} onNavigate={router.navigate} />
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
