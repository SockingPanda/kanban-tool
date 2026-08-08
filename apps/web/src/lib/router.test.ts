import { describe, expect, test, vi } from "vitest"

import { assertCanonicalBoardSlug } from "./board-slug"
import {
  navigateApp,
  navigateDefaultBoard,
  parseAppRoute,
  routePath,
} from "./router"

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
    expect(parseAppRoute("/app/boards/alpha/list")).toEqual({
      kind: "not-found",
      pathname: "/app/boards/alpha/list",
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
})

describe("History API navigation", () => {
  test("resolves a runtime board selector before replacing the home URL", async () => {
    const history = {
      pushState: vi.fn(),
      replaceState: vi.fn(),
    }

    const route = await navigateApp("/app/", {
      defaultBoard: "selector:active",
      resolveBoard: async (selector) => {
        expect(selector).toBe("selector:active")
        return "canonical-board"
      },
      history,
    })

    expect(route).toEqual({
      kind: "board",
      boardSlug: "canonical-board",
      pathname: "/app/boards/canonical-board/board",
    })
    expect(history.replaceState).toHaveBeenCalledWith({}, "", "/app/boards/canonical-board/board")
    expect(history.pushState).not.toHaveBeenCalled()
  })

  test("does not put an unresolved runtime selector into the URL", async () => {
    const history = { pushState: vi.fn(), replaceState: vi.fn() }
    const route = await navigateApp("/app/", { defaultBoard: "selector:active", history })

    expect(route).toEqual({ kind: "home", pathname: "/app/" })
    expect(history.pushState).not.toHaveBeenCalled()
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

  test("renders an invalid explicit board route as an error without history", async () => {
    const history = { pushState: vi.fn(), replaceState: vi.fn() }
    const route = await navigateApp("/app/boards/team%20one/board", { history })

    expect(route).toMatchObject({ kind: "error", code: "invalid-board-slug" })
    expect(history.pushState).not.toHaveBeenCalled()
    expect(history.replaceState).not.toHaveBeenCalled()
  })

  test("renders an invalid resolver result as an error without history", async () => {
    const history = { pushState: vi.fn(), replaceState: vi.fn() }
    const route = await navigateApp("/app/", {
      defaultBoard: "selector:active",
      resolveBoard: async () => "b_reserved",
      history,
    })

    expect(route).toMatchObject({ kind: "error", code: "invalid-board-slug" })
    expect(history.pushState).not.toHaveBeenCalled()
    expect(history.replaceState).not.toHaveBeenCalled()
  })
})
