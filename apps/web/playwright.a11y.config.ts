import { defineConfig, devices } from "@playwright/test"

const foundationViewport = { width: 1440, height: 900 }
const baseURL = process.env.KANBAN_A11Y_BASE_URL

if (baseURL === undefined || baseURL.length === 0) {
  throw new Error("KANBAN_A11Y_BASE_URL is required for the real-host a11y lane")
}

export default defineConfig({
  testDir: "./tests",
  testMatch: "**/a11y-keyboard.spec.ts",
  outputDir: "../../output/playwright/a11y-09c",
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
    reducedMotion: "reduce",
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
  },
  projects: [
    {
      name: "chromium",
      use: {
        ...devices["Desktop Chrome"],
        viewport: foundationViewport,
        locale: "zh-CN",
        colorScheme: "light",
        reducedMotion: "reduce",
      },
    },
    {
      name: "firefox",
      use: {
        ...devices["Desktop Firefox"],
        viewport: foundationViewport,
        locale: "zh-CN",
        colorScheme: "light",
        reducedMotion: "reduce",
      },
    },
  ],
})
