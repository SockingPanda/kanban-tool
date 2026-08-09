import { describe, expect, test } from "vitest"

import { createTranslator } from "../../lib/i18n"
import { HealthReadError } from "../../lib/api/health-read-model"
import { presentHealthError } from "./health-error"

describe("health error presentation", () => {
  test("maps server_unavailable and status without exposing server text", () => {
    const error = new HealthReadError("http", "Web health request failed.", {
      status: 503,
      apiErrorCode: "server_unavailable",
    })
    const copy = presentHealthError(error, createTranslator("en"))

    expect(copy.detail).toContain("503")
    expect(copy.detail).toContain("unavailable")
    expect(copy.detail).not.toContain("/srv/private")
  })

  test("maps malformed responses in both supported locales", () => {
    const error = new HealthReadError("invalid_contract", "Web health response is invalid.")
    const zh = presentHealthError(error, createTranslator("zh"))
    const en = presentHealthError(error, createTranslator("en"))

    expect(zh.detail).toContain("无法识别")
    expect(en.detail).toContain("unrecognized")
    expect(zh.nextStep).toContain("重试")
    expect(en.nextStep).toContain("retry")
  })
})
