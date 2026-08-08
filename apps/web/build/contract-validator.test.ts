import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync } from "node:fs"
import os from "node:os"
import path from "node:path"

import { describe, expect, test } from "vitest"

import validRuntime from "../src/lib/api/generated/fixtures/runtime-web-config-output.valid.json"
import invalidBoard from "../src/lib/api/generated/fixtures/api-board-task-map-response.invalid.json"
import validBoard from "../src/lib/api/generated/fixtures/api-board-task-map-response.valid.json"
import invalidHealth from "../src/lib/api/generated/fixtures/api-health-response.invalid.json"
import validHealth from "../src/lib/api/generated/fixtures/api-health-response.valid.json"

import { createContractValidatorPlugin } from "./contract-validator"

describe("CSP-safe generated contract validator", () => {
  test("compiles the generated schema into a static validator without eval", async () => {
    const plugin = createContractValidatorPlugin()
    const resolved = plugin.resolveId("virtual:kanban-contract-validator/runtime-web-config-output")
    expect(resolved).toBe("\0virtual:kanban-contract-validator/runtime-web-config-output")
    if (typeof resolved !== "string") throw new Error("contract validator virtual module did not resolve")

    const source = plugin.load(resolved)
    expect(source).toEqual(expect.any(String))
    if (typeof source !== "string") throw new Error("contract validator virtual module did not load")

    expect(source).toContain("urn:kanban-tool:schema:runtime:web-config:v1")
    expect(source).not.toContain("new Function")
    expect(source).not.toContain("Ajv2020")

    const module = await import(`data:text/javascript,${encodeURIComponent(source)}`)
    const validator = module.default as (value: unknown) => value is typeof validRuntime
    expect(validator(validRuntime)).toBe(true)
    expect(validator({ ...validRuntime, unexpected: true })).toBe(false)
  })

  test("reads the canonical generated schema artifact", () => {
    const schema = JSON.parse(
      readFileSync(new URL("../src/lib/api/generated/schemas/runtime-web-config-output.schema.json", import.meta.url), "utf8"),
    ) as { $id?: unknown }
    expect(schema.$id).toBe("urn:kanban-tool:schema:runtime:web-config:v1")
  })

  test.each([
    ["api-health-response", validHealth, invalidHealth],
    ["api-board-task-map-response", validBoard, invalidBoard],
  ])("emits static validators for %s", async (slug, valid, invalid) => {
    const plugin = createContractValidatorPlugin()
    const resolved = plugin.resolveId(`virtual:kanban-contract-validator/${slug}`)
    expect(resolved).toBe(`\0virtual:kanban-contract-validator/${slug}`)
    if (typeof resolved !== "string") throw new Error("contract validator virtual module did not resolve")

    const source = plugin.load(resolved)
    expect(source).toEqual(expect.any(String))
    if (typeof source !== "string") throw new Error("contract validator virtual module did not load")
    expect(source).not.toContain("new Function")
    expect(source).not.toContain("Ajv2020")

    const module = await import(`data:text/javascript,${encodeURIComponent(source)}`)
    const validator = module.default as ((value: unknown) => boolean) & { errors?: unknown }
    expect(validator(valid), `${slug} valid`).toBe(true)
    expect(validator(invalid), `${slug} invalid`).toBe(false)
  })

  test("rejects traversal and unknown generated validator slugs", () => {
    const plugin = createContractValidatorPlugin()

    expect(() => plugin.resolveId("virtual:kanban-contract-validator/../runtime-web-config-output")).toThrow(
      /invalid generated contract validator slug/i,
    )
    expect(() => plugin.resolveId("virtual:kanban-contract-validator/Runtime-web-config-output")).toThrow(
      /invalid generated contract validator slug/i,
    )
    expect(() => plugin.resolveId("virtual:kanban-contract-validator/does-not-exist")).toThrow(
      /unknown generated contract validator slug/i,
    )
  })

  test("rejects schema roots reached through a parent-directory symlink", () => {
    const temporaryRoot = mkdtempSync(path.join(os.tmpdir(), "kanban-contract-validator-"))
    const canonicalRoot = path.join(temporaryRoot, "canonical", "schemas")
    const linkedParent = path.join(temporaryRoot, "linked-parent")
    const linkedSchemaDirectory = path.join(linkedParent, "schemas")
    mkdirSync(canonicalRoot, { recursive: true })
    copyFileSync(
      new URL("../src/lib/api/generated/schemas/runtime-web-config-output.schema.json", import.meta.url),
      path.join(canonicalRoot, "runtime-web-config-output.schema.json"),
    )
    symlinkSync(path.dirname(canonicalRoot), linkedParent, "dir")

    try {
      const plugin = createContractValidatorPlugin({ schemaDirectory: linkedSchemaDirectory })
      expect(() => plugin.resolveId("virtual:kanban-contract-validator/runtime-web-config-output")).toThrow(
        /symlink/i,
      )
    } finally {
      rmSync(temporaryRoot, { recursive: true, force: true })
    }
  })
})
