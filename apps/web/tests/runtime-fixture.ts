import { readFileSync } from "node:fs"

import type { Page } from "@playwright/test"

import type { WebRuntimeConfig } from "../src/lib/runtime"

const validRuntime = JSON.parse(
  readFileSync(new URL("../src/lib/api/generated/fixtures/runtime-web-config-output.valid.json", import.meta.url), "utf8"),
) as WebRuntimeConfig

/** 仅用于 Preview 测试 harness：生产 runtime.json 仍由 kanban serve 提供。 */
export async function installRuntimeFixture(page: Page) {
  await page.route("**/app/runtime.json", async (route) => {
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(validRuntime),
    })
  })
}
