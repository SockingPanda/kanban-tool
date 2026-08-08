import { defineConfig, devices } from "@playwright/test"

const foundationViewport = { width: 1440, height: 900 }

export default defineConfig({
  testDir: "./tests",
  outputDir: "./test-results",
  fullyParallel: true,
  forbidOnly: Boolean(process.env.CI),
  retries: process.env.CI ? 2 : 0,
  reporter: process.env.CI ? "github" : "list",
  use: {
    baseURL: "http://127.0.0.1:4173",
    viewport: foundationViewport,
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
  },
  webServer: {
    command: "pnpm vite-build && pnpm preview",
    cwd: ".",
    reuseExistingServer: false,
    timeout: 120_000,
    url: "http://127.0.0.1:4173/app/",
  },
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"], viewport: foundationViewport },
    },
    {
      name: "firefox",
      use: { ...devices["Desktop Firefox"], viewport: foundationViewport },
    },
    {
      // 这里只是上游 WebKit 引擎代理，不是已打包的 Linux WebKitGTK/Tauri smoke test。
      name: "webkit",
      use: { ...devices["Desktop Safari"], viewport: foundationViewport },
      metadata: {
        engineProxyOnly: true,
        packagedWebKitGtk: false,
      },
    },
  ],
})
