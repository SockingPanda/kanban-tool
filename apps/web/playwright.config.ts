import { defineConfig, devices } from "@playwright/test"

const foundationViewport = { width: 1440, height: 900 }
const releaseProof = process.env.KANBAN_RELEASE_PROOF === "1"
const releaseBaseURL = process.env.KANBAN_RELEASE_BASE_URL

if (releaseProof && (releaseBaseURL === undefined || releaseBaseURL.length === 0)) {
  throw new Error("KANBAN_RELEASE_PROOF=1 requires KANBAN_RELEASE_BASE_URL")
}

export default defineConfig({
  testDir: "./tests",
  outputDir: "../../output/playwright",
  fullyParallel: !releaseProof,
  workers: releaseProof ? 1 : undefined,
  forbidOnly: Boolean(process.env.CI),
  retries: releaseProof ? 0 : (process.env.CI ? 2 : 0),
  reporter: process.env.CI ? "github" : "list",
  use: {
    baseURL: releaseBaseURL ?? "http://127.0.0.1:4173",
    viewport: foundationViewport,
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
  },
  testMatch: releaseProof ? ["**/release-proof.spec.ts"] : undefined,
  testIgnore: releaseProof ? undefined : ["**/release-proof.spec.ts"],
  webServer: releaseProof ? undefined : {
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
  ],
})
