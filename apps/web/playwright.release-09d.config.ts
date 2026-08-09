import { defineConfig, devices } from "@playwright/test"

const baseURL = process.env.KANBAN_RELEASE_09D_BASE_URL
if (baseURL === undefined || baseURL.length === 0) {
  throw new Error("09D real-host proof requires KANBAN_RELEASE_09D_BASE_URL")
}

const foundationViewport = { width: 1440, height: 900 }

export default defineConfig({
  testDir: "./tests",
  testMatch: ["**/release-09d-performance.spec.ts"],
  outputDir: "../../output/playwright/release-09d",
  fullyParallel: false,
  workers: 1,
  forbidOnly: true,
  retries: 0,
  reporter: process.env.CI ? "github" : "list",
  expect: { timeout: 120_000 },
  timeout: 30 * 60_000,
  use: {
    baseURL,
    viewport: foundationViewport,
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
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
  ],
})
