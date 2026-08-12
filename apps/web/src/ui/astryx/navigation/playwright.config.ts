import { defineConfig, devices } from "@playwright/test"

export default defineConfig({
  testDir: ".",
  testMatch: "**/*.pw.ts",
  outputDir: "../../../../../../output/playwright/astryx-navigation",
  fullyParallel: false,
  workers: 1,
  reporter: "line",
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
})
