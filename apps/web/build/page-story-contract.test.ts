import { describe, expect, test } from "vitest"

import {
  PAGE_STORY_ROUTES,
  PAGE_STORY_ROUTER_ALIASES,
  PAGE_STORY_ROUTER_BOUNDARIES,
  definePageStoryContract,
  validatePageStoryCatalog,
  validatePageStoryContract,
  validatePageStoryRouterCoverage,
} from "./page-story-contract"

function contract(path = "/app/boards/:boardSlug/board", spec = PAGE_STORY_ROUTES.find((entry) => entry.path === path)!) {
  return definePageStoryContract({
    route: { ...spec, productionOwner: "features/example/ExamplePage" },
    fixture: { canonicalSafe: true, api: false, sse: false, mutation: false },
    responsive: { wide: "wide evidence", narrow: "narrow evidence" },
    states: ["ready", "offline"],
    frame: "workspace",
    astryx: { package: "@astryxdesign/core@0.3.0", commands: ["astryx build example"], adopted: ["production owner"] },
  })
}

describe("PageStoryContract", () => {
  test("accepts canonical metadata and rejects legacy owner-inset", () => {
    const result = validatePageStoryContract(contract())
    expect(result.ok).toBe(true)
    expect(validatePageStoryContract({ ...contract(), frame: "owner-inset" }).errors).toEqual(expect.arrayContaining([
      expect.objectContaining({ path: "frame", code: "invalid-value" }),
    ]))
  })

  test("fails closed when canonical fields conflict with legacy-looking fields", () => {
    const input = {
      ...contract(),
      fixture: { ...contract().fixture, api: true, canonicalSafe: false, network: { api: false }, deterministic: true },
      canonicalSafe: false,
      deterministic: true,
      responsive: null,
    }
    const result = validatePageStoryContract(input)
    expect(result.ok).toBe(false)
    expect(result.errors).toEqual(expect.arrayContaining([
      expect.objectContaining({ path: "fixture.api", code: "network-enabled" }),
      expect.objectContaining({ path: "responsive", code: "invalid-type" }),
    ]))
  })

  test("rejects malformed route kind/view and unknown paths", () => {
    const malformed = validatePageStoryContract({
      ...contract("/app/boards/:boardSlug/list"),
      route: { ...contract("/app/boards/:boardSlug/list").route, kind: "board" },
    })
    expect(malformed.errors).toEqual(expect.arrayContaining([expect.objectContaining({ path: "route", code: "invalid-value" })]))

    const unknown = validatePageStoryContract({ ...contract(), route: { ...contract().route, path: "/app/unknown" } })
    expect(unknown.errors).toEqual(expect.arrayContaining([expect.objectContaining({ path: "route.path", code: "unknown-route" })]))
  })
})

describe("PageStoryCatalog and router coverage", () => {
  const routes = PAGE_STORY_ROUTES.map((route) => ({ contract: contract(route.path), readyStoryId: `story-${route.kind}` }))
  const complete = { routes, aliases: PAGE_STORY_ROUTER_ALIASES, boundaries: PAGE_STORY_ROUTER_BOUNDARIES }

  test("covers formal routes, ready stories, aliases, and boundaries", () => {
    expect(validatePageStoryCatalog(complete, { requireFormalRoutes: true, requireRouterCoverage: true }).ok).toBe(true)
  })

  test("defaults catalog validation to complete route and router coverage", () => {
    const partial = { routes: [routes[0]], aliases: [], boundaries: [] }
    const result = validatePageStoryCatalog(partial)
    expect(result.ok).toBe(false)
    expect(result.errors).toEqual(expect.arrayContaining([
      expect.objectContaining({ path: "routes", code: "missing-route" }),
      expect.objectContaining({ path: "aliases", code: "missing-boundary" }),
      expect.objectContaining({ path: "boundaries", code: "missing-boundary" }),
    ]))
    expect(validatePageStoryCatalog(partial, { requireFormalRoutes: false, requireRouterCoverage: false }).ok).toBe(true)
  })

  test("reports duplicate source index and missing ready story", () => {
    const result = validatePageStoryCatalog({ ...complete, routes: [routes[0], { ...routes[0], readyStoryId: "" }] })
    expect(result.errors).toEqual(expect.arrayContaining([
      expect.objectContaining({ path: "routes[1].contract.route.path", code: "duplicate-value", message: expect.stringContaining("routes[0]") }),
      expect.objectContaining({ path: "routes[1].readyStoryId", code: "missing-ready-story" }),
    ]))
  })

  test("requires exact aliases and exact not-found/invalid-slug boundaries", () => {
    expect(validatePageStoryRouterCoverage({ aliases: PAGE_STORY_ROUTER_ALIASES, boundaries: [
      PAGE_STORY_ROUTER_BOUNDARIES[0],
      { ...PAGE_STORY_ROUTER_BOUNDARIES[1], pathPattern: "/wrong", expectedKind: "not-found", errorCode: undefined },
    ] }).errors).toEqual(expect.arrayContaining([
      expect.objectContaining({ path: "boundaries", code: "missing-boundary" }),
      expect.objectContaining({ path: "boundaries[1]", code: "unknown-route" }),
    ]))
    expect(validatePageStoryRouterCoverage({ aliases: PAGE_STORY_ROUTER_ALIASES, boundaries: PAGE_STORY_ROUTER_BOUNDARIES }).ok).toBe(true)
  })

  test("rejects unknown and duplicate router entries", () => {
    const result = validatePageStoryRouterCoverage({
      aliases: [...PAGE_STORY_ROUTER_ALIASES, PAGE_STORY_ROUTER_ALIASES[0], { aliasKind: "default-board", path: "/other", canonicalPath: "/app/" }],
      boundaries: [...PAGE_STORY_ROUTER_BOUNDARIES, PAGE_STORY_ROUTER_BOUNDARIES[0]],
    })
    expect(result.errors).toEqual(expect.arrayContaining([
      expect.objectContaining({ path: "aliases[1]", code: "duplicate-value", message: expect.stringContaining("aliases[0]") }),
      expect.objectContaining({ path: "aliases[2]", code: "unknown-route" }),
      expect.objectContaining({ path: "boundaries[2]", code: "duplicate-value" }),
    ]))
  })

  test("rejects malformed null and primitive router entries without throwing", () => {
    const result = validatePageStoryRouterCoverage({
      aliases: [null, "bad", ...PAGE_STORY_ROUTER_ALIASES],
      boundaries: [null, 42, ...PAGE_STORY_ROUTER_BOUNDARIES],
    })
    expect(result.errors).toEqual(expect.arrayContaining([
      expect.objectContaining({ path: "aliases[0]", code: "invalid-type" }),
      expect.objectContaining({ path: "aliases[1]", code: "invalid-type" }),
      expect.objectContaining({ path: "boundaries[0]", code: "invalid-type" }),
      expect.objectContaining({ path: "boundaries[1]", code: "invalid-type" }),
    ]))
  })
})
