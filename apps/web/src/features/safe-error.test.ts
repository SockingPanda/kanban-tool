import { describe, expect, test } from "vitest"

import { HttpTransportError } from "../lib/api/http-transport"
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
})
