import { readFileSync } from "node:fs"

import { describe, expect, test } from "vitest"

import validRuntime from "../src/lib/api/generated/fixtures/runtime-web-config-output.valid.json"

import { createRuntimeValidatorPlugin } from "./runtime-validator"

describe("CSP-safe generated runtime validator", () => {
  test("compiles the generated schema into a static validator without eval", async () => {
    const plugin = createRuntimeValidatorPlugin()
    const resolved = plugin.resolveId("virtual:kanban-runtime-validator")
    expect(resolved).toBe("\0virtual:kanban-runtime-validator")
    if (typeof resolved !== "string") throw new Error("runtime validator virtual module did not resolve")

    const source = plugin.load(resolved)
    expect(source).toEqual(expect.any(String))
    if (typeof source !== "string") throw new Error("runtime validator virtual module did not load")

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
})
