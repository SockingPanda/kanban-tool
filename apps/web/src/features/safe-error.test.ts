import { describe, expect, test } from "vitest"

import { HttpTransportError } from "../lib/api/http-transport"
import { ExplorerReadError } from "../lib/api/explorer-read-model"
import { SignalsOntologyReadError } from "../lib/api/signals-ontology-read-model"
import { localizedErrorMessage } from "./safe-error"

describe("safe localized feature errors", () => {
  test("does not render arbitrary exception text", () => {
    expect(localizedErrorMessage(new Error("secret database path"), "Fallback copy", "en")).toBe("Fallback copy")
  })

  test("maps transport API codes to localized safe copy", () => {
    const error = new HttpTransportError("http", "secret server detail", {
      status: 409,
      apiError: { code: "invalid_transition", message: "secret" },
    })
    expect(localizedErrorMessage(error, "fallback", "en")).toContain("conflicted")
    expect(localizedErrorMessage(error, "fallback", "zh")).toContain("冲突")
  })

  test("maps board scope errors without exposing the source message", () => {
    const error = new SignalsOntologyReadError("board_scope", "board=b_other")
    expect(localizedErrorMessage(error, "fallback", "en")).toContain("another board")
    expect(localizedErrorMessage(error, "fallback", "en")).not.toContain("b_other")
  })

  test("maps explorer errors to local copy while retaining allowlisted kind and status", () => {
    const error = new ExplorerReadError("http", "secret server detail", { status: 503 })
    const message = localizedErrorMessage(error, "fallback", "en")

    expect(message).toContain("unreadable response")
    expect(message).toContain("http")
    expect(message).toContain("503")
    expect(message).not.toContain("secret server detail")
  })

  test("keeps explorer offline errors distinct and local", () => {
    const error = new ExplorerReadError("offline", "secret offline detail")

    expect(localizedErrorMessage(error, "fallback", "en")).toContain("local service is unreachable")
    expect(localizedErrorMessage(error, "fallback", "en")).not.toContain("secret offline detail")
  })
})
