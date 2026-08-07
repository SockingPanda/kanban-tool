import { describe, expect, it } from "vitest"

import { normalizeApiBaseUrl, normalizeLoopbackHttpUrl } from "./loopback-url"

describe("loopback URL policy", () => {
  it.each([
    "http://localhost:8721",
    "https://localhost:8721",
    "http://127.0.0.1:8721/",
    "https://[::1]:8721",
  ])("accepts loopback API URL %s", (value) => {
    expect(normalizeApiBaseUrl(value)).toBe(value.replace(/\/+$/, ""))
  })

  it("accepts an explicit same-host proxy path", () => {
    expect(normalizeApiBaseUrl("/__kb_api__/")).toBe("/__kb_api__")
  })

  it.each([
    "http://example.com:8721",
    "https://10.0.0.2:8721",
    "http://localhost:8721/api",
    "http://127.0.0.1:8721@evil.example",
    "//localhost:8721",
    "/",
  ])("rejects non-loopback API URL %s", (value) => {
    expect(() => normalizeApiBaseUrl(value)).toThrow(/VITE_KB_API_BASE_URL.*localhost.*127\.0\.0\.1.*\[::1\].*修正配置/)
  })

  it("applies the same policy to the Vite dev proxy target", () => {
    expect(normalizeLoopbackHttpUrl("http://127.0.0.1:8721/", "VITE_KB_DEV_PROXY_TARGET")).toBe("http://127.0.0.1:8721")
    expect(() => normalizeLoopbackHttpUrl("http://example.com:8721", "VITE_KB_DEV_PROXY_TARGET")).toThrow(
      /VITE_KB_DEV_PROXY_TARGET.*localhost.*修正配置/,
    )
  })
})
