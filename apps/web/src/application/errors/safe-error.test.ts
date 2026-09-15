import { describe, expect, test } from "vitest"

import { RpcTransportError } from "../data/rpc-transport";
import { localizedErrorMessage } from "./safe-error"

describe("safe localized feature errors", () => {
  test("does not render arbitrary exception text", () => {
    expect(localizedErrorMessage(new Error("secret database path"), "Fallback copy", "en")).toBe("Fallback copy")
  })

  test("maps transport API codes to localized safe copy", () => {
    const error = new RpcTransportError("http", "secret server detail", {
      status: 409,
      apiError: { code: "invalid_transition", message: "secret" },
    })
    expect(localizedErrorMessage(error, "fallback", "en")).toContain("conflicted")
    expect(localizedErrorMessage(error, "fallback", "zh")).toContain("冲突")
  })

})
