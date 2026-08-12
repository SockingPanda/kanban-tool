import {spawn, type ChildProcess} from "node:child_process"
import {resolve} from "node:path"

import {expect, test, type Page} from "@playwright/test"

declare global {
  interface Window {
    __astryxFieldLogs: Array<{
      readonly field: string
      readonly kind: string
      readonly value?: string
      readonly reason?: string
    }>
    __astryxFieldRef: HTMLInputElement | null
  }
}

const webRoot = resolve(import.meta.dirname, "../../../../")
const port = 4300 + (process.pid % 500)
const baseUrl = `http://127.0.0.1:${port}`
const harnessPath = "/app/src/ui/astryx/fields/fields.browser.harness.html"
let viteProcess: ChildProcess | undefined

async function waitForHarness(): Promise<void> {
  for (let attempt = 0; attempt < 80; attempt += 1) {
    try {
      const response = await fetch(`${baseUrl}${harnessPath}`)
      if (response.ok) {
        return
      }
    } catch {
      // Vite is still starting.
    }
    await new Promise((resolvePromise) => setTimeout(resolvePromise, 100))
  }
  throw new Error(`Timed out waiting for Vite field harness at ${baseUrl}`)
}

test.beforeAll(async () => {
  viteProcess = spawn(
    "pnpm",
    ["exec", "vite", "--host", "127.0.0.1", "--port", String(port)],
    {cwd: webRoot, stdio: "ignore"},
  )
  await waitForHarness()
})

test.afterAll(() => {
  viteProcess?.kill("SIGTERM")
})

async function openHarness(page: Page): Promise<void> {
  await page.goto(`${baseUrl}${harnessPath}`)
  await expect(page.locator("#field-log")).toBeAttached()
}

async function logs(page: Page): Promise<ReadonlyArray<{field: string; kind: string; value?: string; reason?: string}>> {
  return page.evaluate(() => window.__astryxFieldLogs)
}

test("safe field callbacks, native reset, refs, and IDREFs hold in a real browser", async ({page}) => {
  await openHarness(page)

  const textInput = page.locator("#text-clear")
  await textInput.focus()
  await page.getByRole("button", {name: "Clear text value"}).click()
  await expect(textInput).toHaveValue("")
  await expect(textInput).toBeFocused()
  await expect.poll(async () => logs(page)).toContainEqual({
    field: "text-clear",
    kind: "change",
    value: "",
    reason: "null",
  })

  const validInput = page.locator("#valid-file")
  await validInput.setInputFiles({
    name: "notes.txt",
    mimeType: "text/plain",
    buffer: Buffer.from("notes"),
  })
  await expect(validInput).toHaveValue("")
  await expect(page.locator("#valid-file-file-names")).toHaveText("notes.txt")
  await expect(validInput).toHaveAttribute("aria-describedby", /valid-file-file-names/)
  await expect.poll(async () => page.evaluate(() => window.__astryxFieldRef?.id)).toBe("valid-file")

  await page.getByRole("button", {name: "Clear files"}).click()
  await expect(validInput).toBeFocused()
  await expect.poll(async () => logs(page)).toContainEqual({
    field: "valid-file",
    kind: "change",
    value: "null",
  })

  const validBeforeEmpty = await logs(page)
  await validInput.setInputFiles([])
  await expect.poll(async () => logs(page)).toHaveLength(validBeforeEmpty.length)

  const atomicInput = page.locator("#atomic-file")
  await atomicInput.setInputFiles([
    {name: "ok.txt", mimeType: "text/plain", buffer: Buffer.from("ok")},
    {name: "bad.png", mimeType: "image/png", buffer: Buffer.from("bad")},
  ])
  await expect(atomicInput).toHaveValue("")
  await expect.poll(async () => logs(page)).toContainEqual({
    field: "atomic-file",
    kind: "error",
    reason: "invalid-type",
  })
  await expect.poll(async () => (await logs(page)).filter((entry) => entry.field === "atomic-file" && entry.kind === "change")).toHaveLength(0)
  await expect.poll(async () => (await logs(page)).filter((entry) => entry.field === "atomic-file" && entry.kind === "action")).toHaveLength(0)

  const maxFilesInput = page.locator("#max-files-file")
  await maxFilesInput.setInputFiles([
    {name: "first.txt", mimeType: "text/plain", buffer: Buffer.from("1")},
    {name: "second.txt", mimeType: "text/plain", buffer: Buffer.from("2")},
  ])
  await expect(maxFilesInput).toHaveValue("")
  await expect.poll(async () => logs(page)).toContainEqual({
    field: "max-files-file",
    kind: "error",
    reason: "max-files",
  })
  await expect.poll(async () => (await logs(page)).filter((entry) => entry.field === "max-files-file" && entry.kind !== "error")).toHaveLength(0)

  const sizeInput = page.locator("#size-file")
  await sizeInput.setInputFiles({
    name: "too-large.txt",
    mimeType: "text/plain",
    buffer: Buffer.from("12"),
  })
  await expect(sizeInput).toHaveValue("")
  await expect.poll(async () => logs(page)).toContainEqual({
    field: "size-file",
    kind: "error",
    reason: "size-limit",
  })
  await expect.poll(async () => (await logs(page)).filter((entry) => entry.field === "size-file" && entry.kind !== "error")).toHaveLength(0)

  const throwingInput = page.locator("#throw-file")
  await throwingInput.setInputFiles({
    name: "throw.txt",
    mimeType: "text/plain",
    buffer: Buffer.from("throw"),
  }).catch(() => undefined)
  await expect(throwingInput).toHaveValue("")
  await expect.poll(async () => logs(page)).toContainEqual({
    field: "throw-file",
    kind: "change",
  })

  const throwingValidationInput = page.locator("#throw-validation-file")
  await throwingValidationInput.setInputFiles({
    name: "throw.png",
    mimeType: "image/png",
    buffer: Buffer.from("throw"),
  }).catch(() => undefined)
  await expect(throwingValidationInput).toHaveValue("")
  await expect.poll(async () => logs(page)).toContainEqual({
    field: "throw-validation-file",
    kind: "error",
    reason: "invalid-type",
  })
})
