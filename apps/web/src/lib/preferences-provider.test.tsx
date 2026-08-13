import { renderToStaticMarkup } from "react-dom/server"
import { describe, expect, test } from "vitest"

import { usePreferences } from "./use-preferences"
import { PreferencesProvider } from "./preferences-provider"

function PreferenceProbe() {
  const preferences = usePreferences()
  return (
    <output
      data-testid="sidebar-preference"
      data-step={preferences.sidebarWidthStep}
      data-setter={typeof preferences.setSidebarWidthStep}
      data-resetter={typeof preferences.resetSidebarWidth}
    />
  )
}

describe("PreferencesProvider sidebar width contract", () => {
  test("exposes the default width and explicit update/reset seams", () => {
    const markup = renderToStaticMarkup(
      <PreferencesProvider>
        <PreferenceProbe />
      </PreferencesProvider>,
    )

    expect(markup).toContain('data-step="62"')
    expect(markup).toContain('data-setter="function"')
    expect(markup).toContain('data-resetter="function"')
  })
})
