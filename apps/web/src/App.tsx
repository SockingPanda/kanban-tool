import { Theme } from "@astryxdesign/core/theme"
import { neutralTheme } from "@astryxdesign/theme-neutral/built"

import { ProductShell } from "./ProductShell"
import { usePreferences } from "./lib/use-preferences"
import { PreferencesProvider } from "./lib/preferences-provider"
import { useAppRouter } from "./lib/router"
import { useWebRuntime } from "./lib/runtime-context"

function RuntimeThemedShell() {
  const runtime = useWebRuntime()
  const preferences = usePreferences()
  const router = useAppRouter({
    basePath: runtime.webBasePath,
    defaultBoard: runtime.defaultBoard,
  })

  return (
    <Theme theme={neutralTheme} mode={preferences.theme}>
      <ProductShell
        runtime={runtime}
        route={router.route}
        boundary={router.error ? "error" : undefined}
        error={router.error instanceof Error ? router.error.message : undefined}
        onNavigate={router.navigate}
      />
    </Theme>
  )
}

export default function App() {
  return (
    <PreferencesProvider>
      <RuntimeThemedShell />
    </PreferencesProvider>
  )
}
