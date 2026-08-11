import { useEffect, type ReactNode } from "react"

import { InternationalizationProvider } from "@astryxdesign/core/i18n"
import { Theme } from "@astryxdesign/core/theme"
import { neutralTheme } from "@astryxdesign/theme-neutral/built"
import type { Preview } from "@storybook/react-vite"

import "@astryxdesign/core/reset.css"
import "@astryxdesign/core/astryx.css"
import "@astryxdesign/theme-neutral/theme.css"
import "../src/layers.css"
import "../src/styles.css"

import { astryxMessages, astryxOverrides } from "../src/lib/i18n"

type StorybookLocale = "zh" | "en"
type StorybookTheme = "light" | "dark"
type StorybookDensity = "compact" | "comfortable"

function localeFor(value: unknown): StorybookLocale {
  return value === "en" ? "en" : "zh"
}

function themeFor(value: unknown): StorybookTheme {
  return value === "dark" ? "dark" : "light"
}

function densityFor(value: unknown): StorybookDensity {
  return value === "compact" ? "compact" : "comfortable"
}

// Storybook's preview module is an integration entrypoint, not a refresh boundary.
// eslint-disable-next-line react-refresh/only-export-components
function LocaleSynchronizer({ locale }: { readonly locale: StorybookLocale }) {
  useEffect(() => {
    const root = document.documentElement
    const previous = root.lang
    root.lang = locale
    return () => {
      root.lang = previous
    }
  }, [locale])

  return null
}

// eslint-disable-next-line react-refresh/only-export-components
function DensitySynchronizer({ density }: { readonly density: StorybookDensity }) {
  useEffect(() => {
    const root = document.documentElement
    const previous = root.dataset.density
    root.dataset.density = density
    return () => {
      if (previous === undefined) delete root.dataset.density
      else root.dataset.density = previous
    }
  }, [density])

  return null
}

function withFoundation(Story: () => ReactNode, context: { globals: Record<string, unknown> }) {
  const locale = localeFor(context.globals.locale)
  const mode = themeFor(context.globals.theme)
  const density = densityFor(context.globals.density)

  return (
    <>
      <LocaleSynchronizer locale={locale} />
      <DensitySynchronizer density={density} />
      <InternationalizationProvider locale={locale} messages={astryxMessages} overrides={astryxOverrides}>
        <Theme theme={neutralTheme} mode={mode}>
          <Story />
        </Theme>
      </InternationalizationProvider>
    </>
  )
}

const preview: Preview = {
  globalTypes: {
    locale: {
      description: "Storybook locale",
      defaultValue: "zh",
      toolbar: {
        title: "语言",
        icon: "globe",
        items: [
          { value: "zh", title: "中文" },
          { value: "en", title: "English" },
        ],
      },
    },
    theme: {
      description: "Storybook theme mode",
      defaultValue: "light",
      toolbar: {
        title: "主题",
        icon: "circlehollow",
        items: [
          { value: "light", title: "浅色" },
          { value: "dark", title: "深色" },
        ],
      },
    },
    density: {
      description: "Storybook density",
      defaultValue: "comfortable",
      toolbar: {
        title: "密度",
        icon: "sidebaralt",
        items: [
          { value: "compact", title: "紧凑" },
          { value: "comfortable", title: "舒适" },
        ],
      },
    },
  },
  decorators: [withFoundation],
}

export default preview
