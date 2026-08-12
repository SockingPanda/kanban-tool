import { spawn, type ChildProcess } from "node:child_process"
import { resolve } from "node:path"

import { expect, test } from "@playwright/test"

test.describe.configure({ mode: "serial" })

const webRoot = resolve(import.meta.dirname, "..")

let viteProcess: ChildProcess | undefined
let fixturePort = 1421

const fixtureUrl = () => `http://127.0.0.1:${fixturePort}/app/tests/astryx-navigation.html`

test.beforeAll(async ({ browserName }, testInfo) => {
  void browserName
  const projectPortOffset = testInfo.project.name === "firefox" ? 10 : 0
  fixturePort = 1421 + projectPortOffset + testInfo.workerIndex
  viteProcess = spawn("pnpm", ["exec", "vite", "--host", "127.0.0.1", "--port", String(fixturePort)], {
    cwd: webRoot,
    stdio: "ignore",
  })
  for (let attempt = 0; attempt < 60; attempt += 1) {
    try {
      const response = await fetch(fixtureUrl())
      if (response.ok) return
    } catch {
      // Vite is still starting.
    }
    await new Promise((resolveAttempt) => setTimeout(resolveAttempt, 250))
  }
  throw new Error("Vite navigation browser fixture did not start")
})

test.afterAll(() => {
  viteProcess?.kill()
})

test("SideNav preserves the focused section sibling and removes stale aria-controls", async ({ page }) => {
  await page.goto(fixtureUrl())
  const parent = page.getByTestId("parent-item")
  const nestedChild = page.getByTestId("nested-child")
  const toggle = page.getByRole("button", { name: "收起项目" })

  await expect(parent).toHaveAttribute("aria-controls", "parent-children")
  await expect(toggle.locator('svg[aria-hidden="true"]')).toHaveCount(1)
  await expect(page.locator("#parent-children")).toBeVisible()
  await toggle.click()
  await expect(page.getByRole("button", { name: "展开项目" })).toHaveAttribute("aria-expanded", "false")
  await expect(parent).not.toHaveAttribute("aria-controls", "parent-children")
  await expect(page.locator("#parent-children")).toHaveCount(0)
  await page.getByRole("button", { name: "展开项目" }).click()
  await expect(parent).toHaveAttribute("aria-controls", "parent-children")

  await nestedChild.focus()
  await page.evaluate(() => window.__setSafeNavCollapsed?.(true))
  await expect(page.getByTestId("parent-item")).toBeFocused()
  await expect(page.getByTestId("first-item")).not.toBeFocused()
  await expect(page.getByTestId("parent-item")).not.toHaveAttribute("aria-controls", "parent-children")
  await expect(page.getByRole("button", { name: "展开导航" }).locator('svg[aria-hidden="true"]')).toHaveCount(1)
})

test("TreeList owns keyboard navigation and preserves modified anchor clicks", async ({ page }) => {
  await page.goto(fixtureUrl())
  const tree = page.getByRole("tree")
  const root = page.getByRole("treeitem", { name: /Tree root/ })
  const child = page.getByRole("treeitem", { name: /Tree child/ })
  const rootToggle = page.getByRole("button", { name: "Collapse tree-root" })

  await root.focus()
  await expect(root).toBeFocused()
  await expect(rootToggle.locator('svg[aria-hidden="true"]')).toHaveCount(1)
  await root.press("ArrowDown")
  await expect(child).toBeFocused()
  await child.press("Home")
  await expect(root).toBeFocused()
  await child.focus()
  await child.press("End")
  await expect(child).toBeFocused()
  await child.press("ArrowLeft")
  await expect(root).toBeFocused()
  await root.press("ArrowRight")
  await expect(child).toBeFocused()
  await child.press("Enter")
  await expect(page.getByTestId("tree-action-count")).toHaveText("1")
  await child.press(" ")
  await expect(page.getByTestId("tree-action-count")).toHaveText("2")

  const childAnchor = page.locator('a[href="#tree-child"]')
  await childAnchor.evaluate((anchor) => anchor.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true, button: 0, ctrlKey: true })))
  await expect(page.getByTestId("tree-action-count")).toHaveText("2")
  await expect(tree).toHaveAttribute("role", "tree")
})
