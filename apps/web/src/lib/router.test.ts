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
