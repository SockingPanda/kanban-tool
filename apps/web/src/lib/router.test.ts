import { describe, expect, test, vi } from "vitest"

import { assertCanonicalBoardSlug } from "./board-slug"
import {
  navigateApp,
  navigateDefaultBoard,
  parseAppRoute,
  routePath,
} from "./router"
import { parseTaskMapUrlState } from "../features/explorer/TaskMapView.logic"

describe("App route parser", () => {
  test("parses the app home route and normalizes a trailing slash", () => {
    expect(parseAppRoute("https://kanban.test/app")).toEqual({
      kind: "home",
      pathname: "/app/",
    })
    expect(parseAppRoute("/app/?from=bookmark")).toEqual({
      kind: "home",
      pathname: "/app/",
    })
  })

  test("parses a board route without decoding a slash into a slug", () => {
    const canonicalSlug = assertCanonicalBoardSlug("alpha-team")
    expect(parseAppRoute("/app/boards/alpha%2Dteam/board/")).toEqual({
      kind: "board",
      boardSlug: canonicalSlug,
      pathname: "/app/boards/alpha-team/board",
    })
    expect(parseAppRoute("/app/boards/alpha%2Fteam/board")).toMatchObject({
      kind: "error",
      code: "invalid-board-slug",
    })
  })

  test("parses settings and returns a stable not-found route", () => {
    expect(parseAppRoute("/app/settings")).toEqual({
      kind: "settings",
      pathname: "/app/settings",
    })
    expect(parseAppRoute("/app/boards/alpha/list")).toMatchObject({
      kind: "board",
      boardSlug: "alpha",
      view: "list",
      pathname: "/app/boards/alpha/list",
    })
  })

  test("round-trips the identity-only project overview route", () => {
    const route = parseAppRoute("/app/boards/default/overview")
    expect(route).toEqual({
      kind: "project-overview",
      boardSlug: "default",
      pathname: "/app/boards/default/overview",
    })
    expect(routePath(route)).toBe("/app/boards/default/overview")
  })

  test("parses independent board-scoped health and maintenance routes", () => {
    expect(parseAppRoute("/app/boards/default/health")).toEqual({
      kind: "health",
      boardSlug: "default",
      pathname: "/app/boards/default/health",
    })
    expect(parseAppRoute("/app/boards/default/maintenance/")).toEqual({
      kind: "maintenance",
      boardSlug: "default",
      pathname: "/app/boards/default/maintenance",
    })
    expect(parseAppRoute("/app/boards/default/health?x=1")).toMatchObject({ kind: "health", boardSlug: "default" })
  })

  test("treats a board slug without a view suffix as the default board view", () => {
    expect(parseAppRoute("/app/boards/alpha")).toEqual({
      kind: "board",
      boardSlug: "alpha",
      pathname: "/app/boards/alpha/board",
    })
  })

  test("parses explorer views and preserves URL query state", () => {
    expect(parseAppRoute("http://kanban.test/app/boards/default/list?status=ready&sort=-updated_at&page=2&q=needle")).toMatchObject({
      kind: "board",
      boardSlug: "default",
      view: "list",
      query: "status=ready&sort=-updated_at&page=2&q=needle",
    })
    expect(parseAppRoute("http://kanban.test/app/boards/default/map?task=t_1")).toMatchObject({
      kind: "board",
      view: "map",
      query: "task=t_1",
    })
    expect(parseAppRoute("http://kanban.test/app/boards/default/runs?task=t_1")).toMatchObject({
      kind: "board",
      view: "runs",
      query: "task=t_1",
    })
    expect(parseAppRoute("http://kanban.test/app/boards/default/events?after=10")).toMatchObject({
      kind: "board",
      view: "events",
      query: "after=10",
    })
  })

  test("restores map controls from a copied route query", () => {
    const route = parseAppRoute("http://kanban.test/app/boards/default/map?filter=ready&show_done=true&hide_isolated=true&zoom=1.3&task=t_1")

    expect(route).toMatchObject({ kind: "board", view: "map" })
    if (route.kind !== "board") throw new Error("expected board route")
    expect(parseTaskMapUrlState(route.query ?? "")).toEqual({
      filter: "ready",
      showDoneContext: true,
      hideIsolated: true,
      zoom: 1.3,
      taskId: "t_1",
    })
  })

  test("parses canonical Signals and Ontology board routes with URL filters", () => {
    expect(parseAppRoute("/app/boards/default/signals?status=resolved&kind=agent_cli_friction,agent_timeout&task=default%231&signal=sig_1"))
      .toEqual({
        kind: "board",
        boardSlug: "default",
        pathname: "/app/boards/default/signals",
        view: "signals",
        filters: {
          status: "resolved",
          kinds: ["agent_cli_friction", "agent_timeout"],
          task: "default#1",
          signal: "sig_1",
        },
      })
    expect(parseAppRoute("/app/boards/default/ontology?include_all=true&group_by=candidate_atom&signal=los_1&atom=hash_1")).toEqual({
      kind: "board",
      boardSlug: "default",
      pathname: "/app/boards/default/ontology",
      view: "ontology",
      filters: { includeAll: true, groupBy: "candidate_atom", signal: "los_1", atom: "hash_1" },
    })
    expect(parseAppRoute("/app/boards/default/ontology?group_by=cluster")).toEqual({
      kind: "board",
      boardSlug: "default",
      pathname: "/app/boards/default/ontology",
      view: "ontology",
      filters: { includeAll: false, groupBy: "label" },
    })
  })

  test("serializes feature routes without losing canonical board identity", () => {
    expect(routePath({
      kind: "board",
      boardSlug: assertCanonicalBoardSlug("team-one"),
      view: "signals",
      filters: { status: "open", kinds: ["agent/timeout"], task: "team-one#7", signal: "sig_1" },
    })).toBe("/app/boards/team-one/signals?status=open&kind=agent%2Ftimeout&task=team-one%237&signal=sig_1")
    expect(routePath({
      kind: "board",
      boardSlug: assertCanonicalBoardSlug("team-one"),
      view: "ontology",
      filters: { includeAll: true, groupBy: "proposed_label", signal: "los_1", atom: "hash_1" },
    })).toBe("/app/boards/team-one/ontology?include_all=true&group_by=proposed_label&signal=los_1&atom=hash_1")
  })

  test("serializes operator routes and rejects invalid operator board slugs", () => {
    expect(routePath({ kind: "health", boardSlug: assertCanonicalBoardSlug("team-one") })).toBe("/app/boards/team-one/health")
    expect(routePath({ kind: "maintenance", boardSlug: assertCanonicalBoardSlug("team-one") })).toBe("/app/boards/team-one/maintenance")
    expect(parseAppRoute("/app/boards/team%20one/maintenance")).toMatchObject({ kind: "error", code: "invalid-board-slug" })
  })
})

describe("History API navigation", () => {
  test("keeps the Projects collection at home even when runtime has a board selector", async () => {
    const history = {
      pushState: vi.fn(),
      replaceState: vi.fn(),
    }

    const route = await navigateApp("/app/", {
      defaultBoard: "selector:active",
      resolveBoard: async () => "canonical-board",
      history,
    })

    expect(route).toEqual({ kind: "home", pathname: "/app/" })
    expect(history.pushState).toHaveBeenCalledWith({}, "", "/app/")
    expect(history.replaceState).not.toHaveBeenCalled()
  })

  test("does not call a resolver for an unresolved runtime selector", async () => {
    const history = { pushState: vi.fn(), replaceState: vi.fn() }
    const route = await navigateApp("/app/", { defaultBoard: "selector:active", history })

    expect(route).toEqual({ kind: "home", pathname: "/app/" })
    expect(history.pushState).toHaveBeenCalledWith({}, "", "/app/")
    expect(history.replaceState).not.toHaveBeenCalled()
  })

  test("navigates directly when the caller already has a canonical slug", async () => {
    const history = { pushState: vi.fn(), replaceState: vi.fn() }
    const route = await navigateDefaultBoard(assertCanonicalBoardSlug("default"), { history })

    expect(route).toEqual({ kind: "board", boardSlug: "default", pathname: "/app/boards/default/board" })
    expect(history.replaceState).toHaveBeenCalledWith({}, "", "/app/boards/default/board")
  })

  test("uses pushState for an explicit board navigation", async () => {
    const history = { pushState: vi.fn(), replaceState: vi.fn() }
    const route = await navigateApp({ kind: "board", boardSlug: assertCanonicalBoardSlug("team-one") }, { history })

    expect(routePath(route)).toBe("/app/boards/team-one/board")
    expect(history.pushState).toHaveBeenCalledWith({}, "", "/app/boards/team-one/board")
    expect(history.replaceState).not.toHaveBeenCalled()
  })

  test("preserves explorer view and query for an object deep-link target", async () => {
    const history = { pushState: vi.fn(), replaceState: vi.fn() }
    const target = parseAppRoute("/app/boards/team-one/events?task=t_1&kind=task.updated")

    const route = await navigateApp(target, { history })

    expect(route).toMatchObject({ kind: "board", boardSlug: "team-one", view: "events", query: "task=t_1&kind=task.updated" })
    expect(history.pushState).toHaveBeenCalledWith({}, "", "/app/boards/team-one/events?task=t_1&kind=task.updated")
  })

  test("renders an invalid explicit board route as an error without history", async () => {
    const history = { pushState: vi.fn(), replaceState: vi.fn() }
    const route = await navigateApp("/app/boards/team%20one/board", { history })

    expect(route).toMatchObject({ kind: "error", code: "invalid-board-slug" })
    expect(history.pushState).not.toHaveBeenCalled()
    expect(history.replaceState).not.toHaveBeenCalled()
  })

  test("ignores invalid resolver results because home never resolves defaultBoard", async () => {
    const history = { pushState: vi.fn(), replaceState: vi.fn() }
    const route = await navigateApp("/app/", {
      defaultBoard: "selector:active",
      resolveBoard: async () => "b_reserved",
      history,
    })

    expect(route).toEqual({ kind: "home", pathname: "/app/" })
    expect(history.pushState).toHaveBeenCalledWith({}, "", "/app/")
    expect(history.replaceState).not.toHaveBeenCalled()
  })
})
