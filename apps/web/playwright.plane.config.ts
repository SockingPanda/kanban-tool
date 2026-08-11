import { defineConfig, devices } from "@playwright/test"

const foundationViewport = { width: 1440, height: 900 }
const baseURL = process.env.KANBAN_PLANE_BASE_URL ?? "http://127.0.0.1:4173"
const externalHost = process.env.KANBAN_PLANE_BASE_URL !== undefined

export default defineConfig({
  testDir: "./tests",
  testMatch: "**/plane-*-acceptance.spec.ts",
  outputDir: "../../output/playwright/plane-acceptance",
  fullyParallel: false,
  workers: 1,
  forbidOnly: true,
  retries: 0,
  reporter: process.env.CI ? "github" : "list",
  use: {
    baseURL,
    locale: "zh-CN",
    colorScheme: "light",
    viewport: foundationViewport,
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
  },
  webServer: externalHost ? undefined : {
    command: "pnpm vite-build && pnpm preview",
    cwd: ".",
    reuseExistingServer: false,
    timeout: 120_000,
    url: "http://127.0.0.1:4173/app/",
  },
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"], viewport: foundationViewport, locale: "zh-CN", colorScheme: "light" },
    },
  ],
})
